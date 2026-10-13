#[cfg(target_os = "emscripten")]
use std::os::raw::c_void;

use raylib::{RaylibHandle, RaylibThread};

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_set_main_loop_arg(
        func: extern "C" fn(*mut c_void),
        arg: *mut c_void,
        fps: i32,
        simulate_infinite_loop: i32,
    );
    // fn emscripten_cancel_main_loop();
}

pub fn run<F>(rl: RaylibHandle, thread: RaylibThread, fps: i32, callback: F)
where
    F: FnMut(&mut RaylibHandle, &RaylibThread) + 'static,
{
    #[cfg(target_os = "emscripten")]
    {
        struct State<F> {
            rl: RaylibHandle,
            thread: RaylibThread,
            callback: F,
        }

        extern "C" fn trampoline<F>(arg: *mut c_void)
        where
            F: FnMut(&mut RaylibHandle, &RaylibThread) + 'static,
        {
            // SAFETY: arg comes from Box::into_raw below and is never freed
            let state = unsafe { &mut *(arg as *mut State<F>) };
            (state.callback)(&mut state.rl, &state.thread);
        }

        let arg = Box::into_raw(Box::new(State {
            rl,
            thread,
            callback,
        })) as *mut c_void;
        unsafe { emscripten_set_main_loop_arg(trampoline::<F>, arg, fps, 0) }
    }

    #[cfg(not(target_os = "emscripten"))]
    {
        let (mut rl, mut callback) = (rl, callback);
        if fps > 0 {
            rl.set_target_fps(fps as u32);
        }
        while !rl.window_should_close() {
            callback(&mut rl, &thread);
        }
    }
}
