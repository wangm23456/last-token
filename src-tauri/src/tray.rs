use std::sync::Mutex;

use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, Rect, Size, State,
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    WebviewUrl,
};

pub const TRAY_ID: &str = "last-token-tray";
pub const TRAY_PANEL_LABEL: &str = "tray-panel";
pub const TRAY_PANEL_WIDTH: f64 = 254.0; // ~2/3 of previous 380
pub const TRAY_PANEL_MIN_HEIGHT: f64 = 96.0;
pub const TRAY_PANEL_MAX_HEIGHT: f64 = 520.0;

/// Last tray-icon rect used to re-anchor after content-driven resize.
pub struct TrayPanelAnchor {
    icon: Mutex<Option<(f64, f64, f64, f64)>>,
}

impl Default for TrayPanelAnchor {
    fn default() -> Self {
        Self {
            icon: Mutex::new(None),
        }
    }
}

pub fn clamp_panel_height(height: f64) -> f64 {
    if !height.is_finite() {
        return TRAY_PANEL_MIN_HEIGHT;
    }
    height.clamp(TRAY_PANEL_MIN_HEIGHT, TRAY_PANEL_MAX_HEIGHT)
}

/// Toggle the panel on left or right button release. Press and release of one
/// click are two events, so filtering on Up prevents double toggling. Right
/// click also opens the panel now that the native menu is gone.
pub fn should_toggle_panel(button: MouseButton, button_state: MouseButtonState) -> bool {
    matches!(button, MouseButton::Left | MouseButton::Right)
        && button_state == MouseButtonState::Up
}

/// Pure geometry: anchor the panel to the tray icon inside the monitor work
/// area. Icon in the top half of the work area -> panel below the icon,
/// otherwise above. Horizontally centered on the icon, clamped so the panel
/// never overflows the work area (multi-monitor / negative coords included).
pub fn compute_panel_position(
    icon: (f64, f64, f64, f64),      // x, y, width, height (physical)
    panel: (f64, f64),               // width, height (physical)
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

fn icon_rect_from(rect: &Rect) -> (f64, f64, f64, f64) {
    let (px, py) = match rect.position {
        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
        tauri::Position::Logical(l) => (l.x, l.y),
    };
    let (sw, sh) = match rect.size {
        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
        tauri::Size::Logical(s) => (s.width, s.height),
    };
    (px, py, sw, sh)
}

fn position_panel_at_icon(app: &AppHandle, panel: &tauri::WebviewWindow, icon: (f64, f64, f64, f64)) {
    let panel_size = panel
        .outer_size()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((TRAY_PANEL_WIDTH, TRAY_PANEL_MAX_HEIGHT));
    let monitor = app
        .monitor_from_point(icon.0, icon.1)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let wa = m.work_area();
        let (x, y) = compute_panel_position(
            icon,
            panel_size,
            (
                wa.position.x as f64,
                wa.position.y as f64,
                wa.size.width as f64,
                wa.size.height as f64,
            ),
        );
        let _ = panel.set_position(PhysicalPosition::new(x, y));
    }
}

fn toggle_tray_panel(app: &AppHandle, rect: &Rect) {
    let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.hide();
        return;
    }

    let icon = icon_rect_from(rect);
    if let Some(anchor) = app.try_state::<TrayPanelAnchor>() {
        if let Ok(mut guard) = anchor.icon.lock() {
            *guard = Some(icon);
        }
    }
    position_panel_at_icon(app, &panel, icon);
    let _ = panel.show();
    let _ = panel.set_focus();
}

/// Resize the tray panel to fit measured content height, then re-anchor to the
/// last tray icon click so above-icon layouts stay glued when height changes.
#[tauri::command]
pub fn set_tray_panel_height(
    app: AppHandle,
    height: f64,
    anchor: State<'_, TrayPanelAnchor>,
) -> Result<(), String> {
    let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) else {
        return Ok(());
    };
    let clamped = clamp_panel_height(height);
    panel
        .set_size(Size::Logical(LogicalSize::new(TRAY_PANEL_WIDTH, clamped)))
        .map_err(|e| e.to_string())?;

    if panel.is_visible().unwrap_or(false) {
        if let Ok(guard) = anchor.icon.lock() {
            if let Some(icon) = *guard {
                position_panel_at_icon(&app, &panel, icon);
            }
        }
    }
    Ok(())
}

/// Shared path for the panel footer button: hide the panel first, then
/// surface the main window.
pub fn show_main_window(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) {
        let _ = panel.hide();
    }

    // macOS quirk: when the app is launched from Finder/Dock while the
    // activation policy is `Accessory`, the main window often appears in
    // the wrong space / never surfaces. Toggling to `Regular` first
    // guarantees `show()` paints reliably.
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    }

    match app.get_webview_window("main") {
        Some(window) => {
            if let Err(e) = window.show() {
                eprintln!("[last-token] failed to show main window: {e}");
            }
            if let Err(e) = window.unminimize() {
                eprintln!("[last-token] failed to unminimize main window: {e}");
            }
            if let Err(e) = window.set_focus() {
                eprintln!("[last-token] failed to focus main window: {e}");
            }
        }
        None => {
            eprintln!(
                "[last-token] show_main_window: no `main` window registered yet"
            );
        }
    }
}

fn create_tray_panel(app: &AppHandle) -> Result<(), tauri::Error> {
    WebviewWindowBuilder::new(
        app,
        TRAY_PANEL_LABEL,
        WebviewUrl::App("index.html?surface=tray".into()),
    )
    .title("Last Token")
    .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_MIN_HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .shadow(true)
    .build()?;
    Ok(())
}

pub fn create_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    if app.try_state::<TrayPanelAnchor>().is_none() {
        app.manage(TrayPanelAnchor::default());
    }

    let icon_bytes = include_bytes!("../icons/32x32.png");
    let icon = Image::from_bytes(icon_bytes)?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Last Token")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                rect,
                ..
            } = event
            {
                if should_toggle_panel(button, button_state) {
                    toggle_tray_panel(tray.app_handle(), &rect);
                }
            }
        });

    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.icon(icon).icon_as_template(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        tray_builder = tray_builder.icon(icon);
    }

    let _tray = tray_builder.build(app)?;

    // Compact quota panel anchored to the tray icon (created hidden).
    create_tray_panel(app)?;
    // The main window is defined in `tauri.conf.json` and is already
    // registered before `create_tray` runs. Attach a per-window listener
    // so that when the user closes the main window we restore the menu-bar
    // (Accessory) activation policy and drop the Dock icon. The shared
    // `show_main_window` helper flips to Regular before showing, so this
    // listener is the only place that needs to flip back to Accessory.
    // The global `on_window_event` in `lib.rs` still handles `prevent_close`
    // + `hide` for the same event without conflict.
    if let Some(main_window) = app.get_webview_window("main") {
        let policy_app = app.clone();
        let main_window_clone = main_window.clone();
        main_window.on_window_event(move |event| {
            if !matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                return;
            }
            #[cfg(target_os = "macos")]
            {
                let _ = policy_app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            let _ = main_window_clone.hide();
        });
    }

    Ok(())
}
