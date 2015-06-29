use libc;
use std::mem;
use super::DelegateState;
use Event;
use events::{ Touch, TouchPhase };

use objc::runtime::{ Class, Object, Sel, BOOL, YES };
use objc::declare::{ ClassDecl };

use super::ffi::{
    longjmp,
    id,
    nil,
    CGRect,
    CGPoint,
    CGFloat,
    UIViewAutoresizingFlexibleWidth,
    UIViewAutoresizingFlexibleHeight
 };

use super::jmpbuf;


pub fn create_delegate_class() {
    extern fn did_finish_launching(this: &mut Object, _: Sel, _: id, _: id) -> BOOL {
        unsafe {
            let main_screen: id = msg_send![Class::get("UIScreen").unwrap(), mainScreen];
            let bounds: CGRect = msg_send![main_screen, bounds];
            let scale: CGFloat = msg_send![main_screen, nativeScale];

            let window: id = msg_send![Class::get("UIWindow").unwrap(), alloc];
            let window: id = msg_send![window, initWithFrame:bounds.clone()];

            let size = (bounds.size.width as u32, bounds.size.height as u32);

            let view_controller: id = msg_send![Class::get("MainViewController").unwrap(), alloc];
            let view_controller: id = msg_send![view_controller, init];


            let class = Class::get("MainView").unwrap();
            let view:id = msg_send![class, alloc];
            let view:id = msg_send![view, initForGl:&bounds];


            let _: () = msg_send![view_controller, setView:view];


            let _: () = msg_send![window, setRootViewController:view_controller];

            let _: () = msg_send![window, addSubview:view];
            let _: () = msg_send![window, makeKeyAndVisible];

            let state = Box::new(DelegateState::new(window, view_controller, view, size, scale as f32));
            let state_ptr: *mut DelegateState = mem::transmute(state);
            this.set_ivar("glutinState", state_ptr as *mut libc::c_void);


            let _: () = msg_send![this, performSelector:sel!(postLaunch:) withObject:nil afterDelay:0.0];
        }
        YES
    }

    extern fn post_launch(_: &Object, _: Sel, _: id) {
        unsafe { longjmp(mem::transmute(&mut jmpbuf),1); }
    }

    extern fn did_become_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Focused(true));
        }
    }

    extern fn will_resign_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Focused(false));
        }
    }

    extern fn will_enter_foreground(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Suspended(false));
        }
    }

    extern fn did_enter_background(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Suspended(true));
        }
    }

    extern fn will_terminate(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            // push event to the front to garantee that we'll process it
            // immidiatly after jump
            state.events_queue.push_front(Event::Closed);
            longjmp(mem::transmute(&mut jmpbuf),1);
        }
    }

    extern fn handle_touches(this: &Object, _: Sel, touches: id, _:id) {
        unsafe {
            let state: *mut libc::c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);

            let touches_enum: id = msg_send![touches, objectEnumerator];

            loop {
                let touch: id = msg_send![touches_enum, nextObject];
                if touch == nil {
                    break
                }
                let location: CGPoint = msg_send![touch, locationInView:nil];
                let touch_id = touch as u64;
                let phase: i32 = msg_send![touch, phase];

                state.events_queue.push_back(Event::Touch(Touch {
                    id: touch_id,
                    location: (location.x as f64, location.y as f64),
                    phase: match phase {
                        0 => TouchPhase::Started,
                        1 => TouchPhase::Moved,
                        // 2 is UITouchPhaseStationary and is not expected here
                        3 => TouchPhase::Ended,
                        4 => TouchPhase::Cancelled,
                        _ => panic!("unexpected touch phase: {:?}", phase)
                    }
                }));
            }
        }
    }

    let superclass = Class::get("UIResponder").unwrap();
    let mut decl = ClassDecl::new(superclass, "AppDelegate").unwrap();

    unsafe {
        decl.add_method(sel!(application:didFinishLaunchingWithOptions:),
            did_finish_launching as extern fn(&mut Object, Sel, id, id) -> BOOL);

        decl.add_method(sel!(applicationDidBecomeActive:),
            did_become_active as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillResignActive:),
            will_resign_active as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillEnterForeground:),
            will_enter_foreground as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationDidEnterBackground:),
            did_enter_background as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillTerminate:),
            will_terminate as extern fn(&Object, Sel, id));


        decl.add_method(sel!(touchesBegan:withEvent:),
            handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesMoved:withEvent:),
            handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesEnded:withEvent:),
            handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesCancelled:withEvent:),
            handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));



        decl.add_method(sel!(postLaunch:),
            post_launch as extern fn(&Object, Sel, id));

        decl.add_ivar::<*mut libc::c_void>("glutinState");

        decl.register();
    }
}


pub fn create_view_class() {
    let superclass = Class::get("UIViewController").unwrap();
    let decl = ClassDecl::new(superclass, "MainViewController").unwrap();

    decl.register();

    extern fn init_for_gl(this: &Object, _: Sel, frame: *const libc::c_void) -> id {
        unsafe {
            let bounds: *const CGRect = mem::transmute(frame);
            let view: id = msg_send![this, initWithFrame:(*bounds).clone()];

            let _: () = msg_send![view, setAutoresizingMask: UIViewAutoresizingFlexibleWidth|UIViewAutoresizingFlexibleHeight];
            let _: () = msg_send![view, setAutoresizesSubviews:YES];

            let layer: id = msg_send![view, layer];
            let _ : () = msg_send![layer, setOpaque:YES];

            view
        }
    }

    extern fn layer_class(_: &Class, _: Sel) -> *const Class {
        unsafe { mem::transmute(Class::get("CAEAGLLayer").unwrap()) }
    }


    let superclass = Class::get("UIView").unwrap();
    let mut decl = ClassDecl::new(superclass, "MainView").unwrap();

    unsafe {
        decl.add_method(sel!(initForGl:),
            init_for_gl as extern fn(&Object, Sel, *const libc::c_void) -> id);

        decl.add_class_method(sel!(layerClass),
            layer_class as extern fn(&Class, Sel) -> *const Class);
        decl.register();
    }
}