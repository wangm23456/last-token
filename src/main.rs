//! Last Token: LLM API quota monitoring desktop app.

use std::time::Duration;

use gpui_kit::*;

use last_token_lib::service::Core;
use last_token_lib::ui::shell::AppShell;
use last_token_lib::ui::store::AppStore;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            cx.set_app_identity("com.wangmeng.last-token", "Last Token");
            gpui_kit::init(cx);
            // The product is dark-native: status colors and density were
            // designed against the dark surface.
            gpui_kit::component::theme::Theme::change(
                gpui_kit::component::theme::ThemeMode::Dark,
                None,
                cx,
            );

            let core = Core::new().expect("failed to initialize app core");
            let store = cx.new(|cx| AppStore::new(core.clone(), cx));

            // Background refresh scheduler: one tick per minute, refresh when
            // the snapshot is older than the configured interval. Refresh
            // runs through AppStore so alerts reach the notification center.
            let scheduler_store = store.clone();
            cx.spawn(async move |cx| loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let due = scheduler_store.read_with(cx, |store, _| {
                    if store.busy != last_token_lib::ui::store::Busy::Idle {
                        return false;
                    }
                    let Some(snap) = &store.snapshot else {
                        return true;
                    };
                    let interval_ms = store.settings.refresh_interval_minutes * 60 * 1000;
                    chrono::Utc::now().timestamp_millis() - snap.refreshed_at >= interval_ms
                });
                if due {
                    scheduler_store.update(cx, |store, cx| store.refresh(cx));
                }
            })
            .detach();

            // Initial probe + refresh happen in AppStore::new; kick a network
            // refresh so the first data lands quickly.
            store.update(cx, |store, cx| store.refresh(cx));

            let main = gpui_kit::open_window(
                WindowOptions {
                    window_min_size: Some(size(px(720.), px(520.))),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    let view = cx.new(|cx| AppShell::new(window, cx, store.clone()));
                    last_token_lib::ui::tray::register_main_window(window.window_handle());
                    view
                },
            )
            .expect("failed to open main window");

            // Tray panel and late windows reach the store through a global.
            cx.set_global(last_token_lib::ui::tray::StoreGlobal(store.clone()));
            last_token_lib::ui::tray::setup(cx);

            let _ = main;
        });
}
