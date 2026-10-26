//! Main window shell: header, tab navigation, shared view state.

use gpui_kit::component::notification::Notification;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::{h_flex, v_flex, WindowExt};
use gpui_kit::*;

use crate::domain::PublicAccount;
use crate::ui::providers_tab::AccountForm;
use crate::ui::store::AppStore;

/// Top-level surface of the two-sided IA (KTD1): the watch surface is the
/// default; the management surface carries accounts/alerts/settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Watch,
    Management,
}

/// Add/edit account dialog form state and input handles.
pub struct DeletePending {
    pub account: PublicAccount,
}

pub struct AppShell {
    pub store: Entity<AppStore>,
    pub surface: Surface,
    /// Open account form (Providers tab dialog).
    pub form: Option<AccountForm>,
    pub deleting: Option<PublicAccount>,
    pub clear_confirm: bool,
    _store_observer: Subscription,
}

impl AppShell {
    pub fn new(
        _window: &mut Window,
        cx: &mut Context<Self>,
        store: Entity<AppStore>,
    ) -> Self {
        // Store updates (refresh, selection, settings) repaint the shell and
        // seed the default watch selection once a snapshot exists.
        let _store_observer = cx.observe(&store, |this, store, cx| {
            let needs_default = store.read(cx).selected_account.is_none()
                && store
                    .read(cx)
                    .snapshot
                    .as_ref()
                    .map(|s| !s.accounts.is_empty())
                    .unwrap_or(false);
            if needs_default {
                let first = store
                    .read(cx)
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.accounts.first().map(|a| a.account.id.clone()));
                if let Some(first) = first {
                    store.update(cx, |store, cx| store.select_account(first, cx));
                }
            }
            let _ = this;
            cx.notify();
        });
        Self {
            store,
            surface: Surface::Watch,
            form: None,
            deleting: None,
            clear_confirm: false,
            _store_observer,
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
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drain_toasts(window, cx);
        let theme = cx.theme().clone();
        let lang = self.lang(cx);

        let tab_labels = [
            crate::i18n::tr(&lang, "tabs.watch"),
            crate::i18n::tr(&lang, "tabs.management"),
        ];
        let selected_ix = match self.surface {
            Surface::Watch => 0,
            Surface::Management => 1,
        };

        let tab_bar = TabBar::new("main-tabs")
            .segmented()
            .children(tab_labels.iter().map(|l| Tab::new().label(l.clone())))
            .selected_index(selected_ix)
            .on_click(cx.listener(|this, ix, _, cx| {
                this.surface = match ix {
                    0 => Surface::Watch,
                    _ => Surface::Management,
                };
                cx.notify();
            }));

        // Watch owns its own panes' scrolling; the management surface scrolls
        // as one column.
        let content = match self.surface {
            Surface::Watch => self.render_watch(window, cx),
            Surface::Management => v_flex()
                .id("mgmt-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .gap_6()
                .pr_1()
                .child(self.render_providers(window, cx))
                .child(self.render_settings(window, cx))
                .into_any_element(),
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
                            .flex_1()
                            .min_h_0()
                            .child(content),
                    ),
            )
    }
}
