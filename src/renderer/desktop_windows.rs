use crate::errors::Result;
use anyhow::{bail, Context};
use std::ptr;
use windows_sys::Win32::Foundation::{GetLastError, SetLastError, BOOL, HWND, LPARAM, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetCursorPos, GetSystemMetrics, SendMessageTimeoutW,
    SetParent, SetWindowPos, ShowWindow, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SW_HIDE, SW_SHOW,
};

#[derive(Debug, Clone, Copy)]
pub struct DesktopRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn attach_window_to_desktop(hwnd: HWND) -> Result<()> {
    unsafe {
        let progman = find_window("Progman", None)?;
        spawn_workerw(progman);
        let host = find_desktop_host(progman).context("failed to resolve desktop host window")?;
        if host.kind != DesktopHostKind::WorkerW {
            tracing::warn!(
                host = host.kind.as_str(),
                "WorkerW host unavailable; using fallback desktop host"
            );
        }
        set_parent_checked(hwnd, host.hwnd).with_context(|| {
            format!(
                "SetParent failed for desktop render window (host={})",
                host.kind.as_str()
            )
        })?;
    }
    Ok(())
}

pub fn show_desktop_window(hwnd: HWND, visible: bool) {
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
    }
}

pub fn place_window_over_virtual_desktop(hwnd: HWND) -> Result<DesktopRect> {
    let rect = virtual_desktop_rect();
    if rect.width <= 0 || rect.height <= 0 {
        bail!("virtual desktop has invalid bounds");
    }

    unsafe {
        let ok = SetWindowPos(
            hwnd,
            ptr::null_mut(),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
        );
        if ok == 0 {
            bail!("SetWindowPos failed for desktop render window");
        }
    }

    Ok(rect)
}

pub fn virtual_desktop_rect() -> DesktopRect {
    unsafe {
        DesktopRect {
            x: GetSystemMetrics(SM_XVIRTUALSCREEN),
            y: GetSystemMetrics(SM_YVIRTUALSCREEN),
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

pub fn virtual_cursor_position() -> Option<(f32, f32)> {
    let mut point: POINT = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        return None;
    }
    let rect = virtual_desktop_rect();
    Some(((point.x - rect.x) as f32, (point.y - rect.y) as f32))
}

unsafe fn spawn_workerw(progman: HWND) {
    let mut _result: usize = 0;
    let _ = SendMessageTimeoutW(progman, 0x052C, 0, 0, 0, 1000, &mut _result as *mut usize);
}

unsafe fn find_window(class_name: &str, window_name: Option<&str>) -> Result<HWND> {
    let class = wide_null(class_name);
    let name_buf = window_name.map(wide_null);
    let name_ptr = name_buf.as_ref().map(|n| n.as_ptr()).unwrap_or(ptr::null());
    let hwnd = FindWindowW(class.as_ptr(), name_ptr);
    if hwnd.is_null() {
        bail!("FindWindowW could not find class {class_name}");
    }
    Ok(hwnd)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopHostKind {
    WorkerW,
    ShellHost,
    ProgmanFallback,
}

impl DesktopHostKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WorkerW => "workerw",
            Self::ShellHost => "shell_host",
            Self::ProgmanFallback => "progman_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DesktopHost {
    hwnd: HWND,
    kind: DesktopHostKind,
}

#[derive(Debug, Clone, Copy)]
struct EnumHostCandidates {
    workerw: HWND,
    shell_host: HWND,
}

impl Default for EnumHostCandidates {
    fn default() -> Self {
        Self {
            workerw: ptr::null_mut(),
            shell_host: ptr::null_mut(),
        }
    }
}

unsafe fn find_desktop_host(progman: HWND) -> Option<DesktopHost> {
    let mut candidates = EnumHostCandidates::default();
    let _ = EnumWindows(
        Some(enum_windows_cb),
        (&mut candidates as *mut EnumHostCandidates).cast::<core::ffi::c_void>() as LPARAM,
    );
    choose_desktop_host(candidates, progman)
}

fn choose_desktop_host(candidates: EnumHostCandidates, progman: HWND) -> Option<DesktopHost> {
    if !candidates.workerw.is_null() {
        return Some(DesktopHost {
            hwnd: candidates.workerw,
            kind: DesktopHostKind::WorkerW,
        });
    }
    if !candidates.shell_host.is_null() {
        return Some(DesktopHost {
            hwnd: candidates.shell_host,
            kind: DesktopHostKind::ShellHost,
        });
    }
    if !progman.is_null() {
        return Some(DesktopHost {
            hwnd: progman,
            kind: DesktopHostKind::ProgmanFallback,
        });
    }
    None
}

unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let shell_class = wide_null("SHELLDLL_DefView");
    let shell = FindWindowExW(hwnd, ptr::null_mut(), shell_class.as_ptr(), ptr::null());
    if shell.is_null() {
        return 1;
    }

    let out = lparam as *mut EnumHostCandidates;
    if !out.is_null() && (*out).shell_host.is_null() {
        (*out).shell_host = hwnd;
    }

    let worker_class = wide_null("WorkerW");
    let worker = FindWindowExW(ptr::null_mut(), hwnd, worker_class.as_ptr(), ptr::null());
    if !worker.is_null() {
        if !out.is_null() {
            (*out).workerw = worker;
        }
        return 0;
    }

    1
}

unsafe fn set_parent_checked(hwnd: HWND, parent: HWND) -> Result<()> {
    SetLastError(0);
    let previous_parent = SetParent(hwnd, parent);
    if previous_parent.is_null() {
        let last_error = GetLastError();
        if last_error != 0 {
            bail!("SetParent returned null (win32={last_error})");
        }
    }
    Ok(())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_hwnd(value: isize) -> HWND {
        value as HWND
    }

    #[test]
    fn chooses_workerw_when_available() {
        let candidates = EnumHostCandidates {
            workerw: fake_hwnd(11),
            shell_host: fake_hwnd(22),
        };
        let host = choose_desktop_host(candidates, fake_hwnd(33)).expect("host should exist");
        assert_eq!(host.kind, DesktopHostKind::WorkerW);
        assert_eq!(host.hwnd, fake_hwnd(11));
    }

    #[test]
    fn falls_back_to_shell_host_when_workerw_missing() {
        let candidates = EnumHostCandidates {
            workerw: ptr::null_mut(),
            shell_host: fake_hwnd(22),
        };
        let host = choose_desktop_host(candidates, fake_hwnd(33)).expect("host should exist");
        assert_eq!(host.kind, DesktopHostKind::ShellHost);
        assert_eq!(host.hwnd, fake_hwnd(22));
    }

    #[test]
    fn falls_back_to_progman_when_no_enum_candidates() {
        let candidates = EnumHostCandidates {
            workerw: ptr::null_mut(),
            shell_host: ptr::null_mut(),
        };
        let host = choose_desktop_host(candidates, fake_hwnd(33)).expect("host should exist");
        assert_eq!(host.kind, DesktopHostKind::ProgmanFallback);
        assert_eq!(host.hwnd, fake_hwnd(33));
    }

    #[test]
    fn returns_none_when_all_hosts_missing() {
        let candidates = EnumHostCandidates {
            workerw: ptr::null_mut(),
            shell_host: ptr::null_mut(),
        };
        let host = choose_desktop_host(candidates, ptr::null_mut());
        assert!(host.is_none());
    }
}
