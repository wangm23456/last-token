//! Tray panel: compact window opened from the system tray icon.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::{h_flex, v_flex, Sizable};
use gpui_kit::StatefulInteractiveElement;
use gpui_kit::*;

use crate::domain::{AccountDashboard, CredentialStatus};
use crate::ui::store::{AppStore, Busy};
use crate::ui::tier_view::{status_color, status_text_short, tier_content, worst_tier};

pub struct TrayPanelView {
    pub store: Entity<AppStore>,
}

impl TrayPanelView {
    pub fn new(store: Entity<AppStore>, _cx: &mut Context<Self>) -> Self {
        Self { store }
    }
}

/// Worst tier across all healthy accounts.
fn worst_across(accounts: &[AccountDashboard]) -> Option<(String, crate::domain::RiskState)> {
    let mut worst: Option<(String, crate::domain::RiskState)> = None;
    for acc in accounts {
        if acc.credential_status != CredentialStatus::Valid || acc.error.is_some() {
            continue;
        }
        if let Some(t) = worst_tier(&acc.tiers) {
            let sev = crate::domain::risk_severity(t.forecast.state);
            let better = match &worst {
                None => true,
                Some((_, state)) => sev > crate::domain::risk_severity(*state),
            };
            if better {
                worst = Some((t.quota.label.clone(), t.forecast.state));
            }
        }
    }
    worst
}

impl Render for TrayPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let lang = store.language.clone();
        let snapshot = store.snapshot.clone();
        let busy = store.busy;

        // Header
        let header = h_flex()
            .w_full()
            .px_2()
            .py_1()
            .justify_between()
            .items_center()
            .border_b_1()
            .border_color(theme.colors.border.opacity(0.6))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(crate::i18n::tr(&lang, "app.name")),
            )
            .child(
                Button::new("tray-refresh")
                    .outline()
                    .xsmall()
                    .label(crate::i18n::tr(&lang, "tray.refresh"))
                    .loading(busy == Busy::Refreshing)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.store.update(cx, |store, cx| store.refresh(cx));
                    })),
            );

        let Some(snapshot) = snapshot else {
            return v_flex()
                .size_full()
                .bg(theme.colors.background)
                .child(header)
                .child(
                    div()
                        .px_2()
                        .py_2()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "tray.loading")),
                );
        };

        // Global risk line
        let worst = worst_across(&snapshot.accounts);
        let risk_line = {
            let (label, state) = match &worst {
                Some((label, state)) => (Some(label.clone()), *state),
                None if !snapshot.accounts.is_empty() => (None, crate::domain::RiskState::Safe),
                None => (None, crate::domain::RiskState::UnknownReset),
            };
            let content = match &label {
                Some(l) => format!("{} · {}", status_text_short(&lang, state), l),
                None if !snapshot.accounts.is_empty() => crate::i18n::tr(&lang, "tray.allSafe"),
                None => crate::i18n::tr(&lang, "tray.noProviders"),
            };
            div()
                .w_full()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(theme.colors.border.opacity(0.6))
                .bg(status_color(&theme, state).opacity(0.08))
                .text_xs()
                .text_color(status_color(&theme, state))
                .child(content)
        };

        // Account rows (compact)
        let mut rows = v_flex()
            .id("tray-rows")
            .gap_1()
            .px_1()
            .py_1()
            .flex_1()
            .min_h_0()
            .overflow_scroll();
        if snapshot.accounts.is_empty() {
            rows = rows.child(
                div()
                    .px_2()
                    .py_3()
                    .w_full()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(crate::i18n::tr(&lang, "tray.emptyHint")),
            );
        }
        for acc in &snapshot.accounts {
            let has_error = acc.credential_status != CredentialStatus::Valid || acc.error.is_some();
            let w = worst_tier(&acc.tiers).cloned();
            let accent = if has_error {
                theme.colors.danger
            } else {
                w.as_ref()
                    .map(|t| status_color(&theme, t.forecast.state))
                    .unwrap_or(theme.colors.success)
            };
            let mut body = v_flex().gap_1().child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .min_w_0()
                            .child(div().size_1().rounded_full().bg(accent))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(acc.account.display_name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(match &w {
                                Some(t) if !t.quota.unlimited => {
                                    format!("{:.0}%", t.quota.utilization)
                                }
                                _ => String::new(),
                            }),
                    ),
            );
            if has_error {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.danger.opacity(0.9))
                        .child(
                            acc.error
                                .clone()
                                .unwrap_or_else(|| crate::i18n::tr(&lang, "tray.credentialError")),
                        ),
                );
            } else {
                for tier in crate::ui::tier_view::sorted_tiers(&acc.tiers) {
                    body = body.child(tier_content(&theme, tier, &lang, true));
                }
            }
            rows = rows.child(
                v_flex()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(accent.opacity(0.45))
                    .bg(theme.colors.background.opacity(0.3))
                    .child(body),
            );
        }

        // Footer actions
        let footer = h_flex()
            .w_full()
            .gap_1()
            .px_1()
            .py_1()
            .border_t_1()
            .border_color(theme.colors.border.opacity(0.6))
            .child(
                Button::new("tray-open-main")
                    .primary()
                    .xsmall()
                    .label(crate::i18n::tr(&lang, "tray.openMain"))
                    .on_click(|_, window, cx| {
                        crate::ui::tray_panel::open_main_window(window, cx);
                    }),
            )
            .child(
                Button::new("tray-quit")
                    .outline()
                    .xsmall()
                    .label(crate::i18n::tr(&lang, "tray.quit"))
                    .on_click(|_, _, cx| {
                        cx.quit();
                    }),
            );

        v_flex()
            .size_full()
            .bg(theme.colors.background)
            .text_color(theme.colors.foreground)
            .child(header)
            .child(risk_line)
            .child(rows)
            .child(footer)
    }
}

/// Surface the main window (used by the tray footer button) and close the
/// panel so the two surfaces do not overlap.
pub fn open_main_window(_window: &mut Window, cx: &mut App) {
    crate::ui::tray::close_panel(cx);
    if let Some(handle) = crate::ui::tray::main_window_handle() {
        let _ = handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    }
}
