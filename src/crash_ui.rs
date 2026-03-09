use std::sync::atomic::{AtomicBool, Ordering};

static DIALOG_SHOWN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use crate::debug_capture;
    use std::path::Path;
    use std::ptr;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };

    pub fn install_panic_hook(debug_requested: bool) {
        if debug_requested {
            debug_capture::install_debug_panic_hook();
        }

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            previous_hook(panic_info);
            let panic_summary = panic_summary(panic_info);
            show_panic_dialog(&panic_summary);
        }));
    }

    pub fn show_fatal_error_dialog(error: &str) {
        let message = fatal_error_message(error);
        show_dialog_once("Aura - Fatal Error", &message);
    }

    pub fn show_error_dialog(title: &str, message: &str) {
        if !mark_dialog_shown(&DIALOG_SHOWN) {
            return;
        }
        let title_wide = wide_null(title);
        let message_wide = wide_null(message);
        unsafe {
            MessageBoxW(
                ptr::null_mut(),
                message_wide.as_ptr(),
                title_wide.as_ptr(),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
            );
        }
    }

    pub fn show_panic_dialog(panic_summary: &str) {
        let message = panic_message(panic_summary);
        show_dialog_once("Aura - Unexpected Crash", &message);
    }

    pub fn show_native_crash_dialog(
        exception_code: u32,
        exception_address: usize,
        crash_text_path: Option<&Path>,
    ) {
        let message = native_crash_message(exception_code, exception_address, crash_text_path);
        show_dialog_once("Aura - Native Crash", &message);
    }

    fn show_dialog_once(title: &str, message: &str) {
        if !mark_dialog_shown(&DIALOG_SHOWN) {
            return;
        }
        let title_wide = wide_null(title);
        let message_wide = wide_null(message);
        unsafe {
            MessageBoxW(
                ptr::null_mut(),
                message_wide.as_ptr(),
                title_wide.as_ptr(),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST,
            );
        }
    }

    fn panic_summary(panic_info: &std::panic::PanicHookInfo<'_>) -> String {
        let payload = if let Some(value) = panic_info.payload().downcast_ref::<&str>() {
            (*value).to_string()
        } else if let Some(value) = panic_info.payload().downcast_ref::<String>() {
            value.clone()
        } else {
            "panic payload is unavailable".to_string()
        };

        if let Some(location) = panic_info.location() {
            return format!(
                "{payload} ({file}:{line})",
                file = location.file(),
                line = location.line()
            );
        }

        payload
    }

    fn fatal_error_message(error: &str) -> String {
        format!(
            "Aura encountered a fatal error and must close.\n\n{}\n\n{}",
            error.trim(),
            diagnostics_hint()
        )
    }

    fn panic_message(panic_summary: &str) -> String {
        format!(
            "Aura crashed unexpectedly and must close.\n\nPanic: {}\n\n{}",
            panic_summary.trim(),
            diagnostics_hint()
        )
    }

    fn native_crash_message(
        exception_code: u32,
        exception_address: usize,
        crash_text_path: Option<&Path>,
    ) -> String {
        let details = crash_text_path
            .map(|path| format!("Crash details: {}", path.display()))
            .unwrap_or_else(|| "Crash details file path is unavailable.".to_string());

        format!(
            "Aura hit a native crash and must close.\n\nException code: 0x{exception_code:08X}\nException address: 0x{exception_address:X}\n{details}\n\n{}",
            diagnostics_hint()
        )
    }

    fn diagnostics_hint() -> String {
        match debug_capture::debug_log_path() {
            Ok(path) => format!(
                "For diagnostics, run Aura with --debug and review: {}",
                path.display()
            ),
            Err(_) => "For diagnostics, run Aura with --debug.".to_string(),
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::atomic::AtomicBool;

        #[test]
        fn fatal_error_message_contains_error_text() {
            let message = fatal_error_message("failed to load config");
            assert!(message.contains("failed to load config"));
            assert!(message.contains("must close"));
        }

        #[test]
        fn native_crash_message_includes_path_when_available() {
            let path = Path::new(r"C:\Users\alice\AppData\Local\aura\aura-crash.txt");
            let message = native_crash_message(0xC0000005, 0x1234, Some(path));
            assert!(message.contains("0xC0000005"));
            assert!(message.contains("0x1234"));
            assert!(message.contains("aura-crash.txt"));
        }

        #[test]
        fn mark_dialog_shown_only_allows_first_call() {
            let flag = AtomicBool::new(false);
            assert!(mark_dialog_shown(&flag));
            assert!(!mark_dialog_shown(&flag));
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{
    install_panic_hook, show_error_dialog, show_fatal_error_dialog, show_native_crash_dialog,
};

#[cfg(not(windows))]
pub fn install_panic_hook(_debug_requested: bool) {}

#[cfg(not(windows))]
pub fn show_fatal_error_dialog(_error: &str) {}

#[cfg(not(windows))]
pub fn show_error_dialog(_title: &str, _message: &str) {}

#[cfg(not(windows))]
pub fn show_native_crash_dialog(
    _exception_code: u32,
    _exception_address: usize,
    _crash_text_path: Option<&std::path::Path>,
) {
}

fn mark_dialog_shown(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::SeqCst)
}
