extern crate init = "gl-init-rs";
extern crate libc;
extern crate gl;

use std::io::stdio::stdin;

fn main() {
    // enumerating monitors
    let monitor = {
        for (num, monitor) in init::get_available_monitors().enumerate() {
            println!("Monitor #{}: {}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        let num = from_str(stdin().read_line().unwrap().as_slice().trim())
            .expect("Plase enter a number");
        let monitor = init::get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {}", monitor.get_name());

        monitor
    };

    let window = init::WindowBuilder::new()
        .with_title("Hello world!".to_string())
        .with_fullscreen(monitor)
        .build()
        .unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol) as *const libc::c_void);

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl::GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        println!("{}", window.wait_events().collect::<Vec<init::Event>>());

        gl::Clear(gl::COLOR_BUFFER_BIT);

        window.swap_buffers();
    }
}
