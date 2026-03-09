use crate::errors::Result;
use crate::tray::{format_running_duration, SessionStats, TrayAnchor, TrayEvent};
use crate::updater::UpdaterStatus;
use crate::version;
use anyhow::{anyhow, bail};
use std::mem::size_of;
use std::ptr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT,
    POINT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows_sys::Win32::System::LibraryLoader::{FindResourceW, GetModuleHandleW};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    DrawIconEx, GetCursorPos, GetWindowLongPtrW, InsertMenuItemW, KillTimer, LoadIconW, LoadImageW,
    PeekMessageW, PostMessageW, RegisterClassW, SetForegroundWindow, SetMenuItemInfoW, SetTimer,
    SetWindowLongPtrW, TrackPopupMenu, TranslateMessage, DI_NORMAL, GWLP_USERDATA, HICON,
    IDI_APPLICATION, IMAGE_BITMAP, IMAGE_ICON, LR_CREATEDIBSECTION, LR_DEFAULTSIZE, LR_SHARED,
    MENUITEMINFOW, MFS_DISABLED, MFT_SEPARATOR, MFT_STRING, MIIM_BITMAP, MIIM_FTYPE, MIIM_ID,
    MIIM_STATE, MIIM_STRING, MSG, PM_REMOVE, TPM_LEFTALIGN, TPM_NOANIMATION, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WM_APP, WM_LBUTTONUP, WM_NCCREATE, WM_NCDESTROY, WM_NULL, WM_RBUTTONUP,
    WM_TIMER, WNDCLASSW, WS_EX_NOACTIVATE,
};

const TRAY_ICON_ID: u32 = 1;
const WM_TRAYICON: u32 = WM_APP + 1;
const SINGLE_INSTANCE_MUTEX_NAME: &str = "Local\\aura-tray-single-instance";
const TRAY_ICON_RESOURCE_ID: u16 = 101;
const NEXT_BACKGROUND_ICON_RESOURCE_ID: u16 = 203;
const REFRESH_ICON_RESOURCE_ID: u16 = 204;
const RELOAD_SETTINGS_ICON_RESOURCE_ID: u16 = 205;
const SETTINGS_ICON_RESOURCE_ID: u16 = 201;
const EXIT_ICON_RESOURCE_ID: u16 = 202;
const NEXT_BACKGROUND_ICON_FALLBACK_RESOURCE_ID: u16 = 303;
const REFRESH_ICON_FALLBACK_RESOURCE_ID: u16 = 304;
const RELOAD_SETTINGS_ICON_FALLBACK_RESOURCE_ID: u16 = 305;
const SETTINGS_ICON_FALLBACK_RESOURCE_ID: u16 = 301;
const EXIT_ICON_FALLBACK_RESOURCE_ID: u16 = 302;
const TRAY_COMMAND_NEXT_BACKGROUND: u32 = 1000;
const TRAY_COMMAND_RELOAD_SETTINGS: u32 = 1001;
const TRAY_COMMAND_CHECK_FOR_UPDATES: u32 = 1002;
const TRAY_COMMAND_SETTINGS: u32 = 1004;
const TRAY_COMMAND_EXIT: u32 = 1005;
const TRAY_MENU_REFRESH_TIMER_ID: usize = 1;
const TRAY_MENU_REFRESH_INTERVAL_MS: u32 = 250;
const TRAY_UPDATE_STATUS_MENU_POSITION: u32 = 2;
const MENU_ICON_SIZE: i32 = 16;
const RT_BITMAP_RESOURCE_TYPE: u16 = 2;
const RT_GROUP_ICON_RESOURCE_TYPE: u16 = 14;

pub struct SingleInstanceGuard {
    handle: HANDLE,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

pub fn try_acquire_single_instance() -> Result<Option<SingleInstanceGuard>> {
    let name = wide_null(SINGLE_INSTANCE_MUTEX_NAME);
    let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        bail!("CreateMutexW failed");
    }

    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return Ok(None);
    }

    Ok(Some(SingleInstanceGuard { handle }))
}

pub struct TrayController {
    shutdown_tx: Sender<()>,
    join_handle: Option<JoinHandle<()>>,
}

impl Drop for TrayController {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn spawn(
    event_tx: UnboundedSender<TrayEvent>,
    session_stats: Arc<SessionStats>,
) -> Result<TrayController> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

    let join_handle = thread::spawn(move || {
        if let Err(error) = run_tray_loop(event_tx, session_stats, shutdown_rx, ready_tx) {
            tracing::error!(error = %error, "tray loop failed");
        }
    });

    let ready = ready_rx
        .recv()
        .map_err(|_| anyhow!("tray thread terminated during startup"))?;
    ready?;

    Ok(TrayController {
        shutdown_tx,
        join_handle: Some(join_handle),
    })
}

struct WindowData {
    event_tx: UnboundedSender<TrayEvent>,
    session_stats: Arc<SessionStats>,
    hinstance: HINSTANCE,
    sticky_update_menu_active: bool,
    reopen_menu_requested: bool,
    last_app_update_status: String,
    active_menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    active_menu_has_check_for_updates: bool,
    update_status_menu_text_wide: Vec<u16>,
}

fn run_tray_loop(
    event_tx: UnboundedSender<TrayEvent>,
    session_stats: Arc<SessionStats>,
    shutdown_rx: Receiver<()>,
    ready_tx: Sender<Result<()>>,
) -> Result<()> {
    let class_name = wide_null("aura_tray_window");
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(ptr::null()) };

    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    let atom = unsafe { RegisterClassW(&wnd_class) };
    if atom == 0 {
        let _ = ready_tx.send(Err(anyhow!("RegisterClassW failed")));
        return Ok(());
    }

    let user_data = Box::new(WindowData {
        event_tx,
        session_stats: session_stats.clone(),
        hinstance,
        sticky_update_menu_active: false,
        reopen_menu_requested: false,
        last_app_update_status: session_stats.app_update_status(),
        active_menu: ptr::null_mut(),
        active_menu_has_check_for_updates: false,
        update_status_menu_text_wide: wide_null(""),
    });
    let user_data_ptr = Box::into_raw(user_data);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            hinstance,
            user_data_ptr as *const _,
        )
    };
    if hwnd.is_null() {
        unsafe {
            drop(Box::from_raw(user_data_ptr));
        }
        let _ = ready_tx.send(Err(anyhow!("CreateWindowExW failed")));
        return Ok(());
    }

    let mut nid = create_notify_icon_data(hwnd, hinstance);
    let add_ok = unsafe { Shell_NotifyIconW(NIM_ADD, &mut nid) };
    if add_ok == 0 {
        unsafe {
            DestroyWindow(hwnd);
        }
        let _ = ready_tx.send(Err(anyhow!("Shell_NotifyIconW(NIM_ADD) failed")));
        return Ok(());
    }

    let _ = ready_tx.send(Ok(()));
    tracing::info!("tray icon initialized");

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        while unsafe { PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        thread::sleep(Duration::from_millis(50));
    }

    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &mut nid);
        DestroyWindow(hwnd);
    }
    tracing::info!("tray icon shutdown complete");
    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create_struct =
                lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            if !create_struct.is_null() {
                let data_ptr = (*create_struct).lpCreateParams as isize;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, data_ptr);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        WM_TRAYICON => {
            let event_code = lparam as u32;
            if let Some(data) = get_window_data(hwnd) {
                match event_code {
                    WM_LBUTTONUP => {
                        if let Some(anchor) = tray_anchor_from_cursor_position() {
                            let _ = data.event_tx.send(TrayEvent::OpenSettingsWindow(anchor));
                        }
                    }
                    WM_RBUTTONUP => {
                        show_context_menu(hwnd, data);
                    }
                    _ => {}
                }
            }
            return 0;
        }
        WM_TIMER => {
            if wparam == TRAY_MENU_REFRESH_TIMER_ID {
                if let Some(data) = get_window_data(hwnd) {
                    if data.sticky_update_menu_active {
                        let app_update_status = data.session_stats.app_update_status();
                        if app_update_status != data.last_app_update_status {
                            data.last_app_update_status = app_update_status.clone();
                            if !data.active_menu.is_null()
                                && !update_update_status_menu_row(data, &app_update_status)
                            {
                                tracing::warn!(
                                    "failed to update Update Status tray menu row in place"
                                );
                            }
                            if data.active_menu_has_check_for_updates
                                && !data.active_menu.is_null()
                                && !set_check_for_updates_menu_enabled(
                                    data.active_menu,
                                    !app_update_status_label_is_in_progress(&app_update_status),
                                )
                            {
                                tracing::warn!(
                                    "failed to update Check for Updates tray menu item state in place"
                                );
                            }
                            if app_update_status_label_is_terminal(&app_update_status) {
                                data.sticky_update_menu_active = false;
                            }
                        }
                    }
                }
                return 0;
            }
        }
        WM_NCDESTROY => {
            let ptr_value = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if ptr_value != 0 {
                let ptr = ptr_value as *mut WindowData;
                drop(Box::from_raw(ptr));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn get_window_data(hwnd: HWND) -> Option<&'static mut WindowData> {
    let ptr_value = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr_value == 0 {
        None
    } else {
        Some(&mut *(ptr_value as *mut WindowData))
    }
}

fn create_notify_icon_data(hwnd: HWND, hinstance: HINSTANCE) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = load_tray_icon(hinstance);
    fill_tip(&mut nid.szTip, "aura");
    nid
}

unsafe fn show_context_menu(hwnd: HWND, data: &mut WindowData) {
    let mut anchor_point: POINT = std::mem::zeroed();
    if GetCursorPos(&mut anchor_point) == 0 {
        tracing::warn!("GetCursorPos failed for tray menu");
        return;
    }

    loop {
        data.reopen_menu_requested = false;
        let menu = CreatePopupMenu();
        if menu.is_null() {
            tracing::warn!("CreatePopupMenu failed");
            return;
        }

        let timer_value = data.session_stats.timer_display().to_string();
        let remote_update_value = data.session_stats.remote_update_timer_display().to_string();
        let app_update_value = data.session_stats.app_update_status();
        let images_value = data.session_stats.total_images().to_string();
        let shown_value = data.session_stats.images_shown().to_string();
        let skipped_value = data.session_stats.manual_skips().to_string();
        let running_value = format_running_duration(data.session_stats.running_duration());
        let shader_name_value = data.session_stats.shader_name();
        let app_version_value = version::get_version().full_version_number(false);

        let app_version_label = wide_null(&app_version_value);
        let timer_label = wide_null(&format_stat_row("Timer", &timer_value));
        let remote_update_label =
            wide_null(&format_stat_row("Remote Update", &remote_update_value));
        let update_status_label = wide_null(&format_stat_row("Update Status", &app_update_value));
        let images_label = wide_null(&format_stat_row("Images", &images_value));
        let shown_label = wide_null(&format_stat_row("Shown", &shown_value));
        let skipped_label = wide_null(&format_stat_row("Skipped", &skipped_value));
        let running_label = wide_null(&format_stat_row("Running", &running_value));
        let shader_label = wide_null(&format_stat_row("Shader", &shader_name_value));
        let shader_active = data.session_stats.is_shader_active();
        let show_check_for_updates = app_update_value != UpdaterStatus::Disabled.label()
            && app_update_value != UpdaterStatus::Unsupported.label();
        let allow_check_for_updates = !app_update_status_label_is_in_progress(&app_update_value);
        let next_background_label = wide_null("Next Background");
        let reload_settings_label = wide_null("Reload Settings");
        let check_updates_label = wide_null("Check for Updates");
        let settings_label = wide_null("Settings");
        let exit_label = wide_null("Exit");
        let next_background_icon = load_menu_icon_bitmap(
            data.hinstance,
            NEXT_BACKGROUND_ICON_RESOURCE_ID,
            NEXT_BACKGROUND_ICON_FALLBACK_RESOURCE_ID,
        );
        let refresh_icon = load_menu_icon_bitmap(
            data.hinstance,
            REFRESH_ICON_RESOURCE_ID,
            REFRESH_ICON_FALLBACK_RESOURCE_ID,
        );
        let reload_settings_icon = load_menu_icon_bitmap(
            data.hinstance,
            RELOAD_SETTINGS_ICON_RESOURCE_ID,
            RELOAD_SETTINGS_ICON_FALLBACK_RESOURCE_ID,
        );
        let settings_icon = load_menu_icon_bitmap(
            data.hinstance,
            SETTINGS_ICON_RESOURCE_ID,
            SETTINGS_ICON_FALLBACK_RESOURCE_ID,
        );
        let exit_icon = load_menu_icon_bitmap(
            data.hinstance,
            EXIT_ICON_RESOURCE_ID,
            EXIT_ICON_FALLBACK_RESOURCE_ID,
        );

        let mut position: u32 = 0;
        if !insert_disabled_menu_item(menu, position, app_version_label.as_ptr()) {
            tracing::warn!("failed to add version tray menu item");
        }
        position += 1;
        if !insert_separator_menu_item(menu, position) {
            tracing::warn!("failed to add version separator tray menu item");
        }
        position += 1;
        if !insert_disabled_menu_item(menu, position, update_status_label.as_ptr()) {
            tracing::warn!("failed to add Update Status tray menu item");
        }
        position += 1;
        if !insert_disabled_menu_item(menu, position, running_label.as_ptr()) {
            tracing::warn!("failed to add Running tray menu item");
        }
        position += 1;
        if !insert_separator_menu_item(menu, position) {
            tracing::warn!("failed to add tray stats separator menu item");
        }
        position += 1;
        if shader_active {
            if !insert_disabled_menu_item(menu, position, shader_label.as_ptr()) {
                tracing::warn!("failed to add Shader tray menu item");
            }
            position += 1;
        } else {
            if !insert_disabled_menu_item(menu, position, timer_label.as_ptr()) {
                tracing::warn!("failed to add Timer tray menu item");
            }
            position += 1;
            if !insert_disabled_menu_item(menu, position, remote_update_label.as_ptr()) {
                tracing::warn!("failed to add Remote Update tray menu item");
            }
            position += 1;
            if !insert_disabled_menu_item(menu, position, images_label.as_ptr()) {
                tracing::warn!("failed to add Images tray menu item");
            }
            position += 1;
            if !insert_disabled_menu_item(menu, position, shown_label.as_ptr()) {
                tracing::warn!("failed to add Shown tray menu item");
            }
            position += 1;
            if !insert_disabled_menu_item(menu, position, skipped_label.as_ptr()) {
                tracing::warn!("failed to add Skipped tray menu item");
            }
            position += 1;
        }
        if !insert_separator_menu_item(menu, position) {
            tracing::warn!("failed to add renderer-specific separator tray menu item");
        }
        position += 1;
        if !shader_active {
            if !insert_command_menu_item(
                menu,
                position,
                TRAY_COMMAND_NEXT_BACKGROUND,
                next_background_label.as_ptr(),
                next_background_icon,
            ) {
                tracing::warn!("failed to add Next Background tray menu item");
            }
            position += 1;
        }
        if show_check_for_updates {
            if !insert_command_menu_item(
                menu,
                position,
                TRAY_COMMAND_CHECK_FOR_UPDATES,
                check_updates_label.as_ptr(),
                refresh_icon,
            ) {
                tracing::warn!("failed to add Check for Updates tray menu item");
            } else if !set_check_for_updates_menu_enabled(menu, allow_check_for_updates) {
                tracing::warn!("failed to set Check for Updates tray menu item state");
            }
            position += 1;
        }
        if !insert_command_menu_item(
            menu,
            position,
            TRAY_COMMAND_RELOAD_SETTINGS,
            reload_settings_label.as_ptr(),
            reload_settings_icon,
        ) {
            tracing::warn!("failed to add Reload Settings tray menu item");
        }
        position += 1;
        if !insert_command_menu_item(
            menu,
            position,
            TRAY_COMMAND_SETTINGS,
            settings_label.as_ptr(),
            settings_icon,
        ) {
            tracing::warn!("failed to add Settings tray menu item");
        }
        position += 1;
        if !insert_separator_menu_item(menu, position) {
            tracing::warn!("failed to add separator tray menu item");
        }
        position += 1;
        if !insert_command_menu_item(
            menu,
            position,
            TRAY_COMMAND_EXIT,
            exit_label.as_ptr(),
            exit_icon,
        ) {
            tracing::warn!("failed to add Exit tray menu item");
        }

        let timer_started = if data.sticky_update_menu_active {
            data.last_app_update_status = app_update_value;
            SetTimer(
                hwnd,
                TRAY_MENU_REFRESH_TIMER_ID,
                TRAY_MENU_REFRESH_INTERVAL_MS,
                None,
            ) != 0
        } else {
            false
        };
        if data.sticky_update_menu_active && !timer_started {
            tracing::warn!("failed to start tray menu refresh timer");
            data.sticky_update_menu_active = false;
        }

        SetForegroundWindow(hwnd);
        data.active_menu = menu;
        data.active_menu_has_check_for_updates = show_check_for_updates;
        let selected_command = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NOANIMATION,
            anchor_point.x,
            anchor_point.y,
            0,
            hwnd,
            ptr::null(),
        );
        data.active_menu = ptr::null_mut();
        data.active_menu_has_check_for_updates = false;
        if timer_started {
            KillTimer(hwnd, TRAY_MENU_REFRESH_TIMER_ID);
        }
        if selected_command != 0 {
            handle_tray_command(data, selected_command as u32, anchor_point);
        }
        PostMessageW(hwnd, WM_NULL, 0, 0);

        DestroyMenu(menu);
        cleanup_menu_icon_bitmap(next_background_icon);
        cleanup_menu_icon_bitmap(refresh_icon);
        cleanup_menu_icon_bitmap(reload_settings_icon);
        cleanup_menu_icon_bitmap(settings_icon);
        cleanup_menu_icon_bitmap(exit_icon);

        if data.reopen_menu_requested {
            continue;
        }

        data.sticky_update_menu_active = false;
        break;
    }
}

unsafe fn handle_tray_command(data: &mut WindowData, command_id: u32, anchor_point: POINT) {
    match command_id {
        TRAY_COMMAND_NEXT_BACKGROUND => {
            let _ = data.event_tx.send(TrayEvent::NextWallpaper);
        }
        TRAY_COMMAND_RELOAD_SETTINGS => {
            let _ = data.event_tx.send(TrayEvent::ReloadSettings);
        }
        TRAY_COMMAND_CHECK_FOR_UPDATES => {
            let checking_status = UpdaterStatus::Checking.label().to_string();
            data.session_stats
                .set_app_update_status(checking_status.clone());
            data.last_app_update_status = checking_status;
            data.sticky_update_menu_active = true;
            data.reopen_menu_requested = true;
            let _ = data.event_tx.send(TrayEvent::CheckForUpdates);
        }
        TRAY_COMMAND_SETTINGS => {
            let _ = data
                .event_tx
                .send(TrayEvent::OpenSettingsWindow(tray_anchor_from_point(
                    anchor_point,
                )));
        }
        TRAY_COMMAND_EXIT => {
            let _ = data.event_tx.send(TrayEvent::Exit);
        }
        _ => {}
    }
}

unsafe fn tray_anchor_from_cursor_position() -> Option<TrayAnchor> {
    let mut anchor_point: POINT = std::mem::zeroed();
    if GetCursorPos(&mut anchor_point) == 0 {
        tracing::warn!("GetCursorPos failed for tray click");
        return None;
    }

    Some(tray_anchor_from_point(anchor_point))
}

fn tray_anchor_from_point(point: POINT) -> TrayAnchor {
    TrayAnchor::new(point.x, point.y)
}

unsafe fn insert_command_menu_item(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
    command_id: u32,
    label: *const u16,
    bitmap: HBITMAP,
) -> bool {
    let mut menu_item: MENUITEMINFOW = std::mem::zeroed();
    menu_item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    menu_item.fMask = MIIM_ID | MIIM_STRING | MIIM_FTYPE;
    menu_item.fType = MFT_STRING;
    menu_item.wID = command_id;
    menu_item.dwTypeData = label as *mut u16;

    if !bitmap.is_null() {
        menu_item.fMask |= MIIM_BITMAP;
        menu_item.hbmpItem = bitmap;
    }

    InsertMenuItemW(menu, position, 1, &menu_item) != 0
}

unsafe fn insert_separator_menu_item(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
) -> bool {
    let mut menu_item: MENUITEMINFOW = std::mem::zeroed();
    menu_item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    menu_item.fMask = MIIM_FTYPE;
    menu_item.fType = MFT_SEPARATOR;
    InsertMenuItemW(menu, position, 1, &menu_item) != 0
}

unsafe fn insert_disabled_menu_item(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    position: u32,
    label: *const u16,
) -> bool {
    let mut menu_item: MENUITEMINFOW = std::mem::zeroed();
    menu_item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    menu_item.fMask = MIIM_STRING | MIIM_FTYPE | MIIM_STATE;
    menu_item.fType = MFT_STRING;
    menu_item.fState = MFS_DISABLED;
    menu_item.dwTypeData = label as *mut u16;
    InsertMenuItemW(menu, position, 1, &menu_item) != 0
}

unsafe fn update_update_status_menu_row(data: &mut WindowData, app_update_status: &str) -> bool {
    data.update_status_menu_text_wide =
        wide_null(&format_stat_row("Update Status", app_update_status));

    let mut menu_item: MENUITEMINFOW = std::mem::zeroed();
    menu_item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    menu_item.fMask = MIIM_STRING;
    menu_item.dwTypeData = data.update_status_menu_text_wide.as_mut_ptr();

    SetMenuItemInfoW(
        data.active_menu,
        TRAY_UPDATE_STATUS_MENU_POSITION,
        1,
        &menu_item,
    ) != 0
}

unsafe fn set_check_for_updates_menu_enabled(
    menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
    enabled: bool,
) -> bool {
    let mut menu_item: MENUITEMINFOW = std::mem::zeroed();
    menu_item.cbSize = size_of::<MENUITEMINFOW>() as u32;
    menu_item.fMask = MIIM_STATE;
    menu_item.fState = if enabled { 0 } else { MFS_DISABLED };
    SetMenuItemInfoW(menu, TRAY_COMMAND_CHECK_FOR_UPDATES, 0, &menu_item) != 0
}

fn load_menu_icon_bitmap(
    hinstance: HINSTANCE,
    bitmap_resource_id: u16,
    icon_fallback_resource_id: u16,
) -> HBITMAP {
    let fallback_icon = unsafe {
        LoadImageW(
            hinstance,
            make_int_resource(icon_fallback_resource_id),
            IMAGE_ICON,
            MENU_ICON_SIZE,
            MENU_ICON_SIZE,
            LR_DEFAULTSIZE | LR_SHARED,
        ) as HICON
    };
    if !fallback_icon.is_null() {
        let fallback_bitmap = unsafe { render_icon_to_bitmap(fallback_icon) };
        if !fallback_bitmap.is_null() {
            tracing::debug!(
                bitmap_resource_id,
                icon_fallback_resource_id,
                "loaded tray menu icon from icon resource"
            );
            return fallback_bitmap;
        }
    }
    let icon_load_error = unsafe { GetLastError() };
    let icon_resource_exists = unsafe {
        !FindResourceW(
            hinstance,
            make_int_resource(icon_fallback_resource_id),
            make_int_resource(RT_GROUP_ICON_RESOURCE_TYPE),
        )
        .is_null()
    };

    let bitmap = unsafe {
        LoadImageW(
            hinstance,
            make_int_resource(bitmap_resource_id),
            IMAGE_BITMAP,
            0,
            0,
            LR_CREATEDIBSECTION,
        ) as HBITMAP
    };
    if !bitmap.is_null() {
        tracing::debug!(bitmap_resource_id, "loaded tray menu bitmap resource");
        return bitmap;
    }
    let load_error = unsafe { GetLastError() };
    let bitmap_resource_exists = unsafe {
        !FindResourceW(
            hinstance,
            make_int_resource(bitmap_resource_id),
            make_int_resource(RT_BITMAP_RESOURCE_TYPE),
        )
        .is_null()
    };
    tracing::warn!(
        bitmap_resource_id,
        icon_fallback_resource_id,
        load_error,
        icon_load_error,
        bitmap_resource_exists,
        icon_resource_exists,
        "menu icon load failed; continuing without icon"
    );
    ptr::null_mut()
}

unsafe fn render_icon_to_bitmap(icon: HICON) -> HBITMAP {
    let memory_dc = CreateCompatibleDC(ptr::null_mut());
    if memory_dc.is_null() {
        return ptr::null_mut();
    }

    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = MENU_ICON_SIZE;
    // Negative height creates a top-down DIB.
    bmi.bmiHeader.biHeight = -MENU_ICON_SIZE;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let mut bits = ptr::null_mut();
    let bitmap = CreateDIBSection(
        ptr::null_mut(),
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        ptr::null_mut(),
        0,
    );
    if bitmap.is_null() || bits.is_null() {
        DeleteDC(memory_dc);
        return ptr::null_mut();
    }
    std::ptr::write_bytes(bits, 0, (MENU_ICON_SIZE * MENU_ICON_SIZE * 4) as usize);

    let old_object = SelectObject(memory_dc, bitmap as HGDIOBJ);
    if old_object.is_null() {
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(memory_dc);
        return ptr::null_mut();
    }

    let draw_ok = DrawIconEx(
        memory_dc,
        0,
        0,
        icon,
        MENU_ICON_SIZE,
        MENU_ICON_SIZE,
        0,
        ptr::null_mut(),
        DI_NORMAL,
    );

    SelectObject(memory_dc, old_object);
    DeleteDC(memory_dc);

    if draw_ok == 0 {
        DeleteObject(bitmap as HGDIOBJ);
        return ptr::null_mut();
    }

    bitmap
}

fn cleanup_menu_icon_bitmap(bitmap: HBITMAP) {
    if bitmap.is_null() {
        return;
    }
    unsafe {
        DeleteObject(bitmap as HGDIOBJ);
    }
}

fn load_tray_icon(hinstance: HINSTANCE) -> HICON {
    let custom = unsafe {
        LoadImageW(
            hinstance,
            make_int_resource(TRAY_ICON_RESOURCE_ID),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        ) as HICON
    };
    if !custom.is_null() {
        tracing::info!(
            resource_id = TRAY_ICON_RESOURCE_ID,
            "loaded custom tray icon"
        );
        return custom;
    }

    tracing::warn!(
        resource_id = TRAY_ICON_RESOURCE_ID,
        "custom tray icon not found, falling back to default"
    );
    unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) }
}

fn make_int_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}

fn format_stat_row(label: &str, value: &str) -> String {
    format!("{label}\t{value}")
}

fn app_update_status_label_is_in_progress(status: &str) -> bool {
    status == UpdaterStatus::Checking.label()
        || status == UpdaterStatus::UpdateAvailable.label()
        || status == UpdaterStatus::Installing.label()
}

fn app_update_status_label_is_terminal(status: &str) -> bool {
    status == UpdaterStatus::UpToDate.label()
        || status == UpdaterStatus::Error.label()
        || status == UpdaterStatus::InstalledPendingRestart.label()
}

fn fill_tip(buf: &mut [u16], text: &str) {
    if buf.is_empty() {
        return;
    }
    let mut encoded = text.encode_utf16().collect::<Vec<_>>();
    encoded.truncate(buf.len().saturating_sub(1));
    let len = encoded.len();
    buf[..len].copy_from_slice(&encoded);
    buf[len] = 0;
    for item in &mut buf[(len + 1)..] {
        *item = 0;
    }
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Foundation::POINT;

    #[test]
    fn app_update_status_label_in_progress_set() {
        assert!(app_update_status_label_is_in_progress(
            UpdaterStatus::Checking.label()
        ));
        assert!(app_update_status_label_is_in_progress(
            UpdaterStatus::UpdateAvailable.label()
        ));
        assert!(app_update_status_label_is_in_progress(
            UpdaterStatus::Installing.label()
        ));
        assert!(!app_update_status_label_is_in_progress(
            UpdaterStatus::InstalledPendingRestart.label()
        ));
        assert!(!app_update_status_label_is_in_progress(
            UpdaterStatus::UpToDate.label()
        ));
    }

    #[test]
    fn app_update_status_label_terminal_set() {
        assert!(app_update_status_label_is_terminal(
            UpdaterStatus::UpToDate.label()
        ));
        assert!(app_update_status_label_is_terminal(
            UpdaterStatus::Error.label()
        ));
        assert!(app_update_status_label_is_terminal(
            UpdaterStatus::InstalledPendingRestart.label()
        ));
        assert!(!app_update_status_label_is_terminal(
            UpdaterStatus::Checking.label()
        ));
    }

    #[test]
    fn tray_anchor_from_point_preserves_coordinates() {
        let anchor = tray_anchor_from_point(POINT { x: 320, y: 640 });

        assert_eq!(anchor.x, 320);
        assert_eq!(anchor.y, 640);
    }
}
