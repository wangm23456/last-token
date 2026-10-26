//! System tray icon and panel window toggling.
//!
//! GPUI has no tray API; `gpui-tray` provides the native status item (AppKit /
//! Shell_NotifyIcon / StatusNotifierItem) and dispatches clicks as ordinary
//! GPUI actions on the UI thread — no second event loop, no polling.

use parking_lot::Mutex;

use gpui_kit::*;
use gpui_tray::{Icon, Tray};

use crate::ui::store::AppStore;
use crate::ui::tray_panel::TrayPanelView;

actions!(tray_actions, [TogglePanel]);

static MAIN_WINDOW: Mutex<Option<AnyWindowHandle>> = Mutex::new(None);
static PANEL_WINDOW: Mutex<Option<AnyWindowHandle>> = Mutex::new(None);

/// Panel geometry (logical pixels), matching the tested contract:
/// width 254, height clamped to [96, 520].
pub const TRAY_PANEL_WIDTH: f64 = 254.0;
pub const TRAY_PANEL_MIN_HEIGHT: f64 = 96.0;
pub const TRAY_PANEL_MAX_HEIGHT: f64 = 520.0;

pub fn clamp_panel_height(height: f64) -> f64 {
    if !height.is_finite() {
        return TRAY_PANEL_MIN_HEIGHT;
    }
    height.clamp(TRAY_PANEL_MIN_HEIGHT, TRAY_PANEL_MAX_HEIGHT)
}

/// Pure geometry: anchor the panel to the tray icon inside the monitor work
/// area. Icon in the top half of the work area -> panel below the icon,
/// otherwise above. Horizontally centered on the icon, clamped so the panel
/// never overflows the work area (multi-monitor / negative coords included).
pub fn compute_panel_position(
    icon: (f64, f64, f64, f64),      // x, y, width, height
    panel: (f64, f64),               // width, height
    work_area: (f64, f64, f64, f64), // x, y, width, height
) -> (f64, f64) {
    let (ix, iy, iw, ih) = icon;
    let (pw, ph) = panel;
    let (wx, wy, ww, wh) = work_area;

    let mut x = ix + iw / 2.0 - pw / 2.0;
    x = x.max(wx).min(wx + ww - pw);

    let icon_center_y = iy + ih / 2.0;
    let mut y = if icon_center_y < wy + wh / 2.0 {
        iy + ih
    } else {
        iy - ph
    };
    y = y.max(wy).min(wy + wh - ph);

    (x, y)
}

pub fn register_main_window(handle: AnyWindowHandle) {
    *MAIN_WINDOW.lock() = Some(handle);
}

pub fn main_window_handle() -> Option<AnyWindowHandle> {
    *MAIN_WINDOW.lock()
}

/// Close the tray panel if it is open.
pub fn close_panel(cx: &mut App) {
    if let Some(handle) = PANEL_WINDOW.lock().take() {
        let _ = handle.update(cx, |_, window, _| {
            window.remove_window();
        });
    }
}

/// Keeps the native status item alive for the process lifetime.
pub struct TrayHandle(pub Tray);

impl gpui_kit::Global for TrayHandle {}

pub fn setup(cx: &mut App) {
    // Decode the bundled PNG for the native icon.
    let icon = image::load_from_memory(include_bytes!("../../assets/icons/32x32.png"))
        .ok()
        .and_then(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            Icon::from_rgba(rgba.into_raw(), w, h).ok()
        });
    if let Some(icon) = icon {
        // No menu: on macOS the primary click dispatches the activation
        // action instead of opening a menu, keeping click-to-toggle.
        match Tray::builder()
            .icon(icon)
            .tooltip("Last Token")
            .on_activate(TogglePanel)
            .build(cx)
        {
            Ok(tray) => cx.set_global(TrayHandle(tray)),
            Err(e) => eprintln!("[last-token] tray setup failed: {e}"),
        }
    }

    cx.on_action(|_: &TogglePanel, cx| toggle_panel(cx));
}

fn toggle_panel(cx: &mut App) {
    // Toggle off when the panel is already open.
    if let Some(handle) = PANEL_WINDOW.lock().take() {
        let _ = handle.update(cx, |_, window, _| {
            window.remove_window();
        });
        return;
    }

    // gpui-tray exposes no icon rect; anchor under the menu bar at a nominal
    // top-right position. GPUI has no display-query API, so the work area is
    // a nominal physical screen; bounds are logical (2x scale assumption).
    let work_area = (0.0, 50.0, 5760.0, 2110.0);
    let icon = (work_area.2 * 0.85, 0.0, 24.0, 24.0);
    let (x, y) = compute_panel_position(icon, (TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT), work_area);
    let scale = 2.0f64;
    let bounds = Bounds {
        origin: point(px((x / scale) as f32), px((y / scale) as f32)),
        size: size(
            px(TRAY_PANEL_WIDTH as f32),
            px(TRAY_PANEL_MAX_HEIGHT as f32),
        ),
    };

    let opened = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: true,
            app_owns_titlebar_drag: false,
            inactive_frame_interval: None,
            is_resizable: false,
            is_minimizable: false,
            display_id: None,
            window_background: Default::default(),
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            icon: None,
            tabbing_identifier: None,
        },
        cx,
        |_, cx| {
            let store = cx.global::<StoreGlobal>().0.clone();
            cx.new(|cx| TrayPanelView::new(store, cx))
        },
    );
    if let Ok((handle, _)) = opened {
        *PANEL_WINDOW.lock() = Some(handle);
    }
}

/// Global handle to the shared `AppStore`, so late-created windows (the tray
/// panel) can reach it without threading it through `open_window` closures.
pub struct StoreGlobal(pub Entity<AppStore>);

impl gpui_kit::Global for StoreGlobal {}
