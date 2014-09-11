#![feature(phase)]

#[cfg(target_os = "android")]
#[phase(plugin, link)]
extern crate android_glue;

extern crate gl;
extern crate gl_init;
extern crate libc;

use std::io::stdio::stdin;

#[cfg(target_os = "android")]
android_start!(main)

fn main() {
    // enumerating monitors
    let monitor = {
        for (num, monitor) in gl_init::get_available_monitors().enumerate() {
            println!("Monitor #{}: {}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        let num = from_str(stdin().read_line().unwrap().as_slice().trim())
            .expect("Plase enter a number");
        let monitor = gl_init::get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {}", monitor.get_name());

        monitor
    };

    let window = gl_init::WindowBuilder::new()
        .with_title("Hello world!".to_string())
        .with_fullscreen(monitor)
        .build()
        .unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol));

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl::GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());
    
    {
        let win_size = window.get_inner_size().unwrap();
        gl::Viewport(0, 0, win_size.val0() as libc::c_int, win_size.val1() as libc::c_int);
    }

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        gl::Clear(gl::COLOR_BUFFER_BIT);
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<gl_init::Event>>());
    }
}
