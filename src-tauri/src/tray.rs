use std::collections::HashMap;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindowBuilder,
};
use crate::domain::{apply_account_order, leading_tier, risk_severity, DashboardSnapshot, RiskState, ProviderKind, CredentialStatus};
use crate::AppState;

pub const TRAY_ID: &str = "last-token-tray";
pub const TRAY_PANEL_LABEL: &str = "tray-panel";

pub struct TrayState {
    pub global_item: MenuItem<tauri::Wry>,
    pub account_items: HashMap<String, MenuItem<tauri::Wry>>,
    /// Enabled account ids in the currently rendered menu order.
    pub ordered_account_ids: Vec<String>,
    pub refresh_item: MenuItem<tauri::Wry>,
}

// Format duration in epoch milliseconds to human readable string (e.g. "2h", "45m")
fn format_duration(ms: i64) -> String {
    let secs = ms / 1000;
    if secs <= 0 {
        return "0s".to_string();
    }
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h{}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// Format absolute time margin (resets_at) for global summary
fn format_absolute_time(ms: i64) -> String {
    let datetime = match chrono::DateTime::from_timestamp(ms / 1000, 0) {
        Some(dt) => dt.with_timezone(&chrono::Local),
        None => return "".to_string(),
    };
    
    let now = chrono::Local::now();
    if datetime.date_naive() == now.date_naive() {
        datetime.format("%H:%M").to_string()
    } else {
        datetime.format("%m-%d %H:%M").to_string()
    }
}

fn provider_label(p: ProviderKind) -> &'static str {
    match p {
        ProviderKind::Claude => "Claude",
        ProviderKind::Codex => "Codex",
        ProviderKind::Gemini => "Gemini",
        ProviderKind::Copilot => "Copilot",
        ProviderKind::Kimi => "Kimi",
        ProviderKind::Zhipu => "Zhipu",
        ProviderKind::ZhipuTeam => "Zhipu Team",
        ProviderKind::Minimax => "MiniMax",
        ProviderKind::Zenmux => "ZenMux",
        ProviderKind::Volcengine => "Volcengine",
    }
}

/// Only a left-button release toggles the panel — press and release of one
/// click are two events, so filtering on Up prevents double toggling.
pub fn should_toggle_panel(button: MouseButton, button_state: MouseButtonState) -> bool {
    button == MouseButton::Left && button_state == MouseButtonState::Up
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

fn toggle_tray_panel(app: &AppHandle, rect: &Rect) {
    let Some(panel) = app.get_webview_window(TRAY_PANEL_LABEL) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.hide();
        return;
    }

    let icon = {
        let (px, py) = match rect.position {
            tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
            tauri::Position::Logical(l) => (l.x, l.y),
        };
        let (sw, sh) = match rect.size {
            tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
            tauri::Size::Logical(s) => (s.width, s.height),
        };
        (px, py, sw, sh)
    };
    let panel_size = panel
        .outer_size()
        .map(|s| (s.width as f64, s.height as f64))
        .unwrap_or((380.0, 520.0));
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
    let _ = panel.show();
    let _ = panel.set_focus();
}

/// Shared path for the tray menu item and the panel footer button:
/// hide the panel first, then surface the main window.
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
    .inner_size(380.0, 520.0)
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
    let app_state = app.state::<AppState>();
    
    // Create initial empty menu
    let global_item = MenuItem::with_id(app, "global_summary", "所有套餐安全", false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let refresh_item = MenuItem::with_id(app, "refresh", "刷新中...", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open_main", "打开主界面", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出 Last Token", true, None::<&str>)?;
    
    let menu = Menu::with_items(app, &[
        &global_item,
        &sep1,
        &refresh_item,
        &open_item,
        &quit_item,
    ])?;

    let icon_bytes = include_bytes!("../icons/32x32.png");
    let icon = Image::from_bytes(icon_bytes)?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Last Token")
        .menu(&menu)
        .show_menu_on_left_click(false)
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
        })
        .on_menu_event(|app, event| {
            let id = event.id.0.as_str();
            match id {
                "refresh" => {
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::refresh_all_action(&app_clone).await;
                    });
                }
                "open_main" => {
                    show_main_window(app);
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
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


    // Cache handles
    let mut ts = app_state.tray_state.lock();
    *ts = Some(TrayState {
        global_item,
        account_items: HashMap::new(),
        ordered_account_ids: Vec::new(),
        refresh_item,
    });

    Ok(())
}

pub fn update_tray_menu(app: &AppHandle, snapshot: &DashboardSnapshot) -> Result<(), tauri::Error> {
    let app_state = app.state::<AppState>();

    // Align native tray rows with overview/tray-panel order.
    let account_order = app_state
        .db
        .get_settings()
        .map(|s| s.account_order)
        .unwrap_or_default();
    let ordered_accounts = apply_account_order(&snapshot.accounts, &account_order);
    let ordered_enabled_ids: Vec<String> = ordered_accounts
        .iter()
        .filter(|a| a.account.enabled)
        .map(|a| a.account.id.clone())
        .collect();

    // Rebuild when membership OR display order changes.
    let rebuild_needed = {
        let ts_guard = app_state.tray_state.lock();
        match &*ts_guard {
            Some(ts) => ts.ordered_account_ids != ordered_enabled_ids,
            None => true,
        }
    };

    if rebuild_needed {
        // Rebuild full menu hierarchy
        let global_item = MenuItem::with_id(app, "global_summary", "所有套餐安全", false, None::<&str>)?;
        let sep1 = PredefinedMenuItem::separator(app)?;
        
        let mut account_items = HashMap::new();
        let mut menu_items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = Vec::new();
        menu_items.push(Box::new(global_item.clone()));
        menu_items.push(Box::new(sep1));
        
        // Add row for each enabled account in shared overview order
        for acc in &ordered_accounts {
            if !acc.account.enabled {
                continue;
            }
            let id = format!("acc_row_{}", acc.account.id);
            let item = MenuItem::with_id(app, &id, &format!("{} - 载入中...", acc.account.display_name), false, None::<&str>)?;
            account_items.insert(acc.account.id.clone(), item.clone());
            menu_items.push(Box::new(item));
        }
        
        let sep2 = PredefinedMenuItem::separator(app)?;
        menu_items.push(Box::new(sep2));
        
        let refresh_text = if snapshot.refresh_in_progress { "刷新中..." } else { "立即刷新" };
        let refresh_item = MenuItem::with_id(app, "refresh", refresh_text, !snapshot.refresh_in_progress, None::<&str>)?;
        menu_items.push(Box::new(refresh_item.clone()));
        
        let open_item = MenuItem::with_id(app, "open_main", "打开主界面", true, None::<&str>)?;
        menu_items.push(Box::new(open_item));
        let quit_item = MenuItem::with_id(app, "quit", "退出 Last Token", true, None::<&str>)?;
        menu_items.push(Box::new(quit_item));
        
        let menu_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = menu_items.iter().map(|item| item.as_ref()).collect();
        let new_menu = Menu::with_items(app, &menu_refs)?;
        
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            let _ = tray.set_menu(Some(new_menu));
        }
        
        let mut ts_guard = app_state.tray_state.lock();
        *ts_guard = Some(TrayState {
            global_item,
            account_items,
            ordered_account_ids: ordered_enabled_ids,
            refresh_item,
        });
    }

    // Now, update labels in place using the cached handles
    let ts_guard = app_state.tray_state.lock();
    let ts = match &*ts_guard {
        Some(t) => t,
        None => return Ok(()),
    };

    // 1. Update Global Summary Item
    // Global copy rules:
    // - 最早风险: <provider> <tier> · <time> (for exhausted/at-risk)
    // - N 个周期待确认 (unknown_reset states)
    // - N 个账户查询失败 (error states)
    // - 正在学习消耗速度 (learning states)
    // - 所有套餐安全 (safe states)
    //
    // "Worst" across accounts is selected by `risk_severity` (exhausted >
    // at_risk > unknown_reset > error > learning > safe); a tie is broken
    // by the earliest exhaustion/reset time. This keeps an Exhausted tier
    // ahead of a less-severe AtRisk tier even when the latter would
    // exhaust first by raw time.
    let mut worst_severity: i32 = -1;
    let mut earliest_risk_time = i64::MAX;
    let mut earliest_risk_info: Option<(ProviderKind, &str, bool)> = None;
    let mut unknown_reset_count = 0;
    let mut error_count = 0;
    let mut learning_count = 0;

    for acc in &snapshot.accounts {
        if !acc.account.enabled {
            continue;
        }
        if acc.credential_status != CredentialStatus::Valid {
            error_count += 1;
            continue;
        }
        if acc.error.is_some() {
            error_count += 1;
            continue;
        }
        for tier in &acc.tiers {
            match tier.forecast.state {
                RiskState::Exhausted | RiskState::AtRisk => {
                    let severity = risk_severity(tier.forecast.state);
                    let ex_at = tier.forecast.exhaustion_at.unwrap_or(i64::MAX);
                    let is_exhausted = matches!(tier.forecast.state, RiskState::Exhausted);
                    if severity > worst_severity
                        || (severity == worst_severity && ex_at < earliest_risk_time)
                    {
                        worst_severity = severity;
                        earliest_risk_time = ex_at;
                        earliest_risk_info =
                            Some((acc.account.provider, tier.quota.id.as_str(), is_exhausted));
                    }
                }
                RiskState::UnknownReset => {
                    unknown_reset_count += 1;
                }
                RiskState::Learning => {
                    learning_count += 1;
                }
                RiskState::Safe | RiskState::Error => {}
            }
        }
    }

    let global_text = if let Some((prov, tier_id, is_exhausted)) = earliest_risk_info {
        let label = provider_label(prov);
        let time_str = format_absolute_time(earliest_risk_time);
        if is_exhausted {
            format!("最早风险: {} {} · 已耗尽", label, tier_id)
        } else {
            format!("最早风险: {} {} · {}", label, tier_id, time_str)
        }
    } else if unknown_reset_count > 0 {
        format!("{} 个周期待确认", unknown_reset_count)
    } else if error_count > 0 {
        format!("{} 个账户查询失败", error_count)
    } else if learning_count > 0 {
        "正在学习消耗速度".to_string()
    } else {
        "所有套餐安全".to_string()
    };

    let _ = ts.global_item.set_text(&global_text);

    // 2. Update each enabled account's worst tier row
    for acc in &snapshot.accounts {
        if !acc.account.enabled {
            continue;
        }
        let menu_item = match ts.account_items.get(&acc.account.id) {
            Some(i) => i,
            None => continue,
        };

        let label = &acc.account.display_name;

        if acc.credential_status != CredentialStatus::Valid {
            let err_msg = match acc.credential_status {
                CredentialStatus::Expired => "凭据已过期",
                CredentialStatus::NotFound => "凭据未找到",
                CredentialStatus::ParseError => "凭据解析失败",
                CredentialStatus::Unavailable => "凭据服务不可用",
                _ => "凭据错误",
            };
            let _ = menu_item.set_text(&format!("🔴 {}: {}", label, err_msg));
            continue;
        }

        if let Some(ref err) = acc.error {
            let _ = menu_item.set_text(&format!("🔴 {}: {}", label, err));
            continue;
        }

        if acc.tiers.is_empty() {
            let _ = menu_item.set_text(&format!("⚪ {}: 无额度信息", label));
            continue;
        }

        // Find worst tier (shared severity semantics)
        let Some(worst_tier) = leading_tier(&acc.tiers) else {
            let _ = menu_item.set_text(&format!("⚪ {}: 无额度信息", label));
            continue;
        };

        let u_val = worst_tier.quota.utilization;
        let t_id = &worst_tier.quota.id;
        
        let row_text = match worst_tier.forecast.state {
            RiskState::Exhausted => {
                format!("🔴 {}: {} 已耗尽", label, t_id)
            }
            RiskState::AtRisk => {
                let dur_str = worst_tier.forecast.exhaustion_at
                    .map(|e_at| format_duration(e_at - now_millis()))
                    .unwrap_or_else(|| "0s".to_string());
                format!("🔴 {}: {} {:.1}% · {}后耗尽", label, t_id, u_val, dur_str)
            }
            RiskState::UnknownReset => {
                format!("⚪ {}: {} {:.1}% · 重置未知", label, t_id, u_val)
            }
            RiskState::Learning => {
                format!("🔵 {}: {} {:.1}% · 学习速度中", label, t_id, u_val)
            }
            RiskState::Safe => {
                format!("🟢 {}: {} {:.1}% · 安全", label, t_id, u_val)
            }
            RiskState::Error => {
                format!("🔴 {}: {} 错误", label, t_id)
            }
        };

        let _ = menu_item.set_text(&row_text);
    }

    // 3. Update refresh item state
    let refresh_label = if snapshot.refresh_in_progress { "刷新中..." } else { "立即刷新" };
    let _ = ts.refresh_item.set_text(refresh_label);
    let _ = ts.refresh_item.set_enabled(!snapshot.refresh_in_progress);

    Ok(())
}
