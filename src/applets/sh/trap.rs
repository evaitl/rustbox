use std::sync::atomic::{AtomicU8, Ordering};

static PENDING_SIGNAL: AtomicU8 = AtomicU8::new(0);

extern "C" fn on_signal(sig: libc::c_int) {
    PENDING_SIGNAL.store(sig as u8, Ordering::SeqCst);
}

pub fn install_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGHUP, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
}

pub fn take_pending_signal() -> Option<i32> {
    let sig = PENDING_SIGNAL.swap(0, Ordering::SeqCst);
    if sig == 0 {
        None
    } else {
        Some(sig as i32)
    }
}

pub fn signal_name(sig: i32) -> Option<&'static str> {
    match sig {
        libc::SIGINT => Some("INT"),
        libc::SIGHUP => Some("HUP"),
        libc::SIGTERM => Some("TERM"),
        _ => None,
    }
}

pub fn reset_handler(sig: i32) {
    unsafe {
        libc::signal(sig, libc::SIG_DFL);
    }
}

pub fn set_handler(sig: i32) {
    unsafe {
        libc::signal(sig, on_signal as *const () as libc::sighandler_t);
    }
}
