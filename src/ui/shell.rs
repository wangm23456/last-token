//! Main window shell: header, tab navigation, shared view state.

use gpui_kit::component::button::Button;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::Sizable;
use gpui_kit::component::{h_flex, v_flex, WindowExt};
use gpui_kit::*;

use crate::domain::PublicAccount;
use crate::ui::providers_tab::AccountForm;
use crate::ui::store::{AppStore, Busy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPage {
    Overview,
    Providers,
    Settings,
}

/// Add/edit account dialog form state and input handles.
pub struct DeletePending {
    pub account: PublicAccount,
}

pub struct AppShell {
    pub store: Entity<AppStore>,
    pub page: TabPage,
    /// Open account form (Providers tab dialog).
    pub form: Option<AccountForm>,
    pub deleting: Option<PublicAccount>,
    pub clear_confirm: bool,
}

impl AppShell {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>, store: Entity<AppStore>) -> Self {
        Self {
            store,
            page: TabPage::Overview,
            form: None,
            deleting: None,
            clear_confirm: false,
        }
    }

    pub fn t(&self, cx: &App, key: &str) -> String {
        self.store.read(cx).t(key)
    }

    pub fn lang(&self, cx: &App) -> String {
        self.store.read(cx).language.clone()
    }

    /// Drain one-shot toasts into window notifications. Called from render:
    /// queue draining is idempotent and notifies nothing, so it cannot loop.
    pub fn drain_toasts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let toasts: Vec<_> = self
            .store
            .update(cx, |store, _| std::mem::take(&mut store.toasts));
        for toast in toasts {
            let note = if toast.error {
                Notification::error(toast.message)
            } else {
                Notification::success(toast.message)
            };
            window.push_notification(note, cx);
        }
    }

    pub(crate) fn render_toolbar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let lang = store.language.clone();
        let refreshed = store
            .snapshot
            .as_ref()
            .map(|s| s.refreshed_at)
            .map(|_| ())
            .is_some();
        let refreshed_at = store.snapshot.as_ref().map(|s| s.refreshed_at);
        let has_custom_order = !store.settings.account_order.is_empty();
        let busy = store.busy;

        let time_str = refreshed_at.map(|ms| {
            let dt = chrono::DateTime::from_timestamp_millis(ms)
                .unwrap_or_else(chrono::Utc::now)
                .with_timezone(&chrono::Local);
            dt.format("%H:%M:%S").to_string()
        });
        let _ = refreshed;

        let mut actions = h_flex().gap_2().items_center();
        if has_custom_order {
            let reset_label = crate::i18n::tr(&lang, "overview.resetOrder");
            actions = actions.child(
                Button::new("reset-order")
                    .outline()
                    .small()
                    .label(reset_label)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.store.update(cx, |store, cx| {
                            store.update_account_order(Vec::new(), cx);
                        });
                        window.refresh();
                        let _ = this;
                    })),
            );
        }
        actions = actions
            .child(
                Button::new("refresh")
                    .outline()
                    .small()
                    .label(crate::i18n::tr(&lang, "overview.refreshAria"))
                    .loading(busy == Busy::Refreshing)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.store.update(cx, |store, cx| store.refresh(cx));
                    })),
            )
            .child(
                Button::new("go-settings")
                    .outline()
                    .small()
                    .label(crate::i18n::tr(&lang, "overview.settingsAria"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = TabPage::Settings;
                        cx.notify();
                    })),
            );

        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "overview.header")),
                    )
                    .child(actions),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(match time_str {
                        Some(t) => crate::i18n::tr_params(
                            &lang,
                            "overview.refreshedAt",
                            &[("time", t.as_str())],
                        ),
                        None => String::new(),
                    }),
            )
            .into_any_element()
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drain_toasts(window, cx);
        let theme = cx.theme().clone();
        let lang = self.lang(cx);

        let tab_labels = [
            crate::i18n::tr(&lang, "tabs.overview"),
            crate::i18n::tr(&lang, "tabs.providers"),
            crate::i18n::tr(&lang, "tabs.settings"),
        ];
        let selected_ix = match self.page {
            TabPage::Overview => 0,
            TabPage::Providers => 1,
            TabPage::Settings => 2,
        };

        let tab_bar = TabBar::new("main-tabs")
            .segmented()
            .children(tab_labels.iter().map(|l| Tab::new().label(l.clone())))
            .selected_index(selected_ix)
            .on_click(cx.listener(|this, ix, _, cx| {
                this.page = match ix {
                    0 => TabPage::Overview,
                    1 => TabPage::Providers,
                    _ => TabPage::Settings,
                };
                cx.notify();
            }));

        let content = match self.page {
            TabPage::Overview => self.render_overview(window, cx),
            TabPage::Providers => self.render_providers(window, cx),
            TabPage::Settings => self.render_settings(window, cx),
        };

        v_flex()
            .size_full()
            .bg(theme.colors.background)
            .text_color(theme.colors.foreground)
            .child(
                h_flex()
                    .w_full()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(theme.colors.border.opacity(0.6))
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child(crate::i18n::tr(&lang, "app.name")),
                            )
                            .child(
                                div()
                                    .size_2()
                                    .rounded_full()
                                    .bg(theme.colors.success)
                                    .opacity(0.9),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.colors.muted_foreground)
                                    .child(crate::i18n::tr(&lang, "app.running")),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .gap_4()
                    .child(tab_bar)
                    .child(
                        v_flex()
                            .id("main-scroll")
                            .flex_1()
                            .min_h_0()
                            // The tab content owns vertical scrolling; the
                            // tab bar and header stay fixed.
                            .overflow_y_scroll()
                            .child(content),
                    ),
            )
    }
}
