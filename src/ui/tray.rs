//! System tray icon (KTD5): native status item via `gpui-tray`; activation
//! focuses the main window. Notifications stay on the system channel (R11);
//! the panel window and its geometry are gone (R10).

use parking_lot::Mutex;

use gpui_kit::*;

// Tray activation focuses the registered main window.
actions!(tray_actions, [OpenMain]);

static MAIN_WINDOW: Mutex<Option<AnyWindowHandle>> = Mutex::new(None);

pub fn register_main_window(handle: AnyWindowHandle) {
    *MAIN_WINDOW.lock() = Some(handle);
}

pub fn main_window_handle() -> Option<AnyWindowHandle> {
    *MAIN_WINDOW.lock()
}

pub fn setup(cx: &mut App) {
    // Decode the bundled PNG for the native icon.
    let icon = image::load_from_memory(include_bytes!("../../assets/icons/32x32.png"))
        .ok()
        .and_then(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            gpui_tray::Icon::from_rgba(rgba.into_raw(), w, h).ok()
        });
    if let Some(icon) = icon {
        match gpui_tray::Tray::builder()
            .icon(icon)
            .tooltip("Last Token")
            .on_activate(OpenMain)
            .build(cx)
        {
            Ok(tray) => cx.set_global(TrayHandle(tray)),
            Err(e) => eprintln!("[last-token] tray setup failed: {e}"),
        }
    }

    cx.on_action(|_: &OpenMain, cx| {
        if let Some(handle) = main_window_handle() {
            let _ = handle.update(cx, |_, window, _| {
                window.activate_window();
            });
        }
    });
}

/// Keeps the native status item alive for the process lifetime.
pub struct TrayHandle(pub gpui_tray::Tray);

impl gpui_kit::Global for TrayHandle {}
