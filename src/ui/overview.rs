//! Overview tab: risk hero, account cards, tier detail dialog.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::Sizable;
use gpui_kit::component::WindowExt;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::{AccountDashboard, RiskState, TierDashboard};
use crate::ui::shell::AppShell;
use crate::ui::store::AppStore;
use crate::ui::tier_view::{
    format_absolute_quota, format_time_margin, sorted_tiers, status_color, status_text,
    tier_content, utilization_color, worst_tier,
};

/// Detail dialog content: one account's tiers plus the selected tier's history.
pub struct TierDetail {
    store: Entity<AppStore>,
    account: AccountDashboard,
    selected_tier: String,
}

impl TierDetail {
    pub fn new(
        store: Entity<AppStore>,
        account: AccountDashboard,
        selected_tier: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut detail = Self {
            store,
            account,
            selected_tier,
        };
        detail.load_history(cx);
        detail
    }

    fn load_history(&mut self, cx: &mut Context<Self>) {
        let account_id = self.account.account.id.clone();
        let tier_id = self.selected_tier.clone();
        self.store.update(cx, |store, cx| {
            store.load_history(account_id, tier_id, cx);
        });
    }

    fn active_tier(&self) -> Option<&TierDashboard> {
        self.account
            .tiers
            .iter()
            .find(|t| t.quota.id == self.selected_tier)
            .or_else(|| sorted_tiers(&self.account.tiers).first().copied())
    }
}

impl Render for TierDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let lang = store.language.clone();
        let history = store
            .history
            .as_ref()
            .filter(|(a, t, _)| a == &self.account.account.id && t == &self.selected_tier)
            .map(|(_, _, pts)| pts.clone())
            .unwrap_or_default();

        let Some(tier) = self.active_tier().cloned() else {
            return div().into_any_element();
        };

        let stat = |label: String, value: String| {
            v_flex()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(theme.colors.border.opacity(0.5))
                .bg(theme.colors.background.opacity(0.4))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                )
        };

        let unlimited_label = crate::i18n::tr(&lang, "overview.unlimitedQuota");
        let usage = if tier.quota.unlimited {
            unlimited_label.clone()
        } else {
            format!("{:.1}%", tier.quota.utilization)
        };
        let absolute = if tier.quota.unlimited {
            unlimited_label.clone()
        } else {
            format_absolute_quota(&tier.quota).unwrap_or_else(|| "—".to_string())
        };
        let burn = if tier.quota.unlimited {
            "—".to_string()
        } else {
            crate::i18n::tr_params(
                &lang,
                "format.ratePerHour",
                &[("rate", &format!("{:.1}", tier.forecast.rate_per_hour))],
            )
        };
        let reset = if tier.quota.resets_at.is_some() {
            format_time_margin(&lang, tier.quota.resets_at)
        } else {
            crate::i18n::tr(&lang, "status.unknown")
        };
        let projected = if tier.quota.unlimited {
            "—".to_string()
        } else {
            format!("{:.1}%", tier.forecast.projected_utilization_at_reset)
        };
        let samples = crate::i18n::tr_params(
            &lang,
            "overview.samplesCount",
            &[
                ("count", &tier.forecast.sample_count.to_string()),
                ("minutes", &tier.forecast.observation_minutes.to_string()),
            ],
        );
        let exhaustion = match (tier.quota.unlimited, tier.forecast.exhaustion_at) {
            (false, Some(ms)) => chrono::DateTime::from_timestamp_millis(ms)
                .map(|d| {
                    d.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|| "—".to_string()),
            _ => "—".to_string(),
        };

        // Tier switcher (only when the account has multiple tiers).
        let switcher = {
            let tiers = sorted_tiers(&self.account.tiers);
            if tiers.len() > 1 {
                let mut row = h_flex().gap_2().flex_wrap();
                for t in tiers {
                    let id = t.quota.id.clone();
                    let label = t.quota.label.clone();
                    let active = id == self.selected_tier;
                    row = row.child(
                        div()
                            .id(format!("tier-switch-{id}"))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .cursor_pointer()
                            .border_color(if active {
                                theme.colors.border
                            } else {
                                theme.colors.border.opacity(0.6)
                            })
                            .bg(if active {
                                theme.colors.button_secondary
                            } else {
                                theme.colors.background.opacity(0.3)
                            })
                            .text_xs()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_tier = id.clone();
                                this.load_history(cx);
                                cx.notify();
                            }))
                            .child(label),
                    );
                }
                row.into_any_element()
            } else {
                div().into_any_element()
            }
        };

        let chart = if history.is_empty() {
            div()
                .h(px(140.))
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.colors.muted_foreground)
                .child(crate::i18n::tr(&lang, "overview.historyEmptyTitle"))
                .into_any_element()
        } else {
            LineChart::new(history.clone())
                .x(|p| p.sampled_at.to_string())
                .y(|p| p.utilization)
                .stroke(utilization_color(&theme, &tier))
                .grid(true)
                .y_domain(0.0, 100.0)
                .into_any_element()
        };

        v_flex()
            .gap_4()
            .child(switcher)
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(stat(
                        crate::i18n::tr(&lang, "overview.windowType"),
                        tier.quota.label.clone(),
                    ))
                    .child(stat(crate::i18n::tr(&lang, "overview.currentUsage"), usage))
                    .child(stat(
                        crate::i18n::tr(&lang, "overview.absoluteQuota"),
                        absolute,
                    ))
                    .child(stat(crate::i18n::tr(&lang, "overview.burnRate"), burn))
                    .child(stat(crate::i18n::tr(&lang, "overview.resetPeriod"), reset))
                    .child(stat(
                        crate::i18n::tr(&lang, "overview.projectedAtReset"),
                        projected,
                    ))
                    .child(stat(crate::i18n::tr(&lang, "overview.samples"), samples))
                    .child(stat(
                        crate::i18n::tr(&lang, "overview.expectedExhaustion"),
                        exhaustion,
                    )),
            )
            .when(tier.forecast.state == RiskState::AtRisk, |col| {
                col.child(
                    v_flex()
                        .gap_1()
                        .p_2()
                        .rounded_md()
                        .bg(theme.colors.danger.opacity(0.08))
                        .border_1()
                        .border_color(theme.colors.danger.opacity(0.15))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.colors.danger)
                                .child(crate::i18n::tr(&lang, "overview.riskWarning")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.colors.danger.opacity(0.85))
                                .child(crate::i18n::tr_params(
                                    &lang,
                                    "overview.riskWarningDesc",
                                    &[
                                        ("rate", &format!("{:.1}", tier.forecast.rate_per_hour)),
                                        (
                                            "time",
                                            &format_time_margin(&lang, tier.forecast.exhaustion_at),
                                        ),
                                        (
                                            "margin",
                                            &format_time_margin(&lang, tier.forecast.exhaustion_at),
                                        ),
                                    ],
                                )),
                        ),
                )
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "overview.historyTitle")),
                    )
                    .child(div().h(px(140.)).w_full().child(chart)),
            )
            .into_any_element()
    }
}

/// Move an account one slot up (`dir = -1`) or down (`dir = 1`) in the
/// snapshot order, then persist the manual order.
fn move_account(shell: &mut AppShell, id: &str, dir: isize, cx: &mut Context<AppShell>) {
    shell.store.update(cx, |store, cx| {
        let Some(snap) = store.snapshot.as_ref() else {
            return;
        };
        let mut ids: Vec<String> = snap.accounts.iter().map(|a| a.account.id.clone()).collect();
        let Some(ix) = ids.iter().position(|i| i == id) else {
            return;
        };
        let target = ix as isize + dir;
        if target < 0 || target as usize >= ids.len() {
            return;
        }
        ids.swap(ix, target as usize);
        store.update_account_order(ids, cx);
    });
}

impl AppShell {
    pub fn render_overview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let lang = store.language.clone();
        let snapshot = store.snapshot.clone();

        let toolbar = self.render_toolbar(window, cx);

        let Some(snapshot) = snapshot else {
            return v_flex()
                .gap_4()
                .child(toolbar)
                .child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "tray.loading")),
                )
                .into_any_element();
        };

        let has_accounts = !snapshot.accounts.is_empty();

        // Risk hero: worst tier across accounts (errors rank above resets).
        let mut worst: Option<(&AccountDashboard, &TierDashboard)> = None;
        let mut worst_sev = -1;
        let mut credential_invalid = false;
        for acc in &snapshot.accounts {
            if acc.credential_status != crate::domain::CredentialStatus::Valid
                || acc.error.is_some()
            {
                credential_invalid = true;
                continue;
            }
            if let Some(t) = worst_tier(&acc.tiers) {
                let sev = crate::domain::risk_severity(t.forecast.state);
                let better = match worst {
                    None => true,
                    Some((_, w)) => {
                        let wsev = crate::domain::risk_severity(w.forecast.state);
                        sev > wsev || (sev == wsev && t.quota.utilization > w.quota.utilization)
                    }
                };
                if better {
                    worst = Some((acc, t));
                    worst_sev = sev;
                }
            }
        }
        let _ = worst_sev;

        let hero = if !has_accounts {
            div().into_any_element()
        } else {
            let (title, desc, color) = match worst {
                Some((acc, tier)) => {
                    let headline = format!("{} · {}", acc.account.display_name, tier.quota.label);
                    let desc = match tier.forecast.state {
                        RiskState::Exhausted => {
                            crate::i18n::tr(&lang, "overview.earliestRiskExhausted")
                        }
                        RiskState::AtRisk => crate::i18n::tr_params(
                            &lang,
                            "overview.earliestRiskAtRisk",
                            &[(
                                "time",
                                &format_time_margin(&lang, tier.forecast.exhaustion_at),
                            )],
                        ),
                        RiskState::Learning => {
                            crate::i18n::tr(&lang, "overview.earliestRiskLearning")
                        }
                        _ if tier.quota.unlimited => {
                            crate::i18n::tr(&lang, "overview.earliestRiskUnlimitedSafe")
                        }
                        _ => crate::i18n::tr(&lang, "overview.earliestRiskSafe"),
                    };
                    (headline, desc, status_color(&theme, tier.forecast.state))
                }
                None if credential_invalid => (
                    crate::i18n::tr(&lang, "overview.credentialInvalid"),
                    crate::i18n::tr(&lang, "overview.credentialInvalidDesc"),
                    theme.colors.danger,
                ),
                None => (
                    crate::i18n::tr(&lang, "overview.allSafe"),
                    crate::i18n::tr(&lang, "overview.allSafeDesc"),
                    theme.colors.success,
                ),
            };

            v_flex()
                .gap_1()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.colors.border.opacity(0.7))
                .bg(theme.colors.background.opacity(0.45))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "overview.earliestRisk")),
                )
                .child(div().text_sm().font_weight(FontWeight::BOLD).child(title))
                .child(div().text_xs().text_color(color).child(desc))
                .into_any_element()
        };

        let empty = if !has_accounts {
            v_flex()
                .gap_2()
                .p_8()
                .items_center()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(crate::i18n::tr(&lang, "overview.emptyTitle")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "overview.emptyDesc")),
                )
                .child(
                    Button::new("empty-add")
                        .label(crate::i18n::tr(&lang, "overview.addProvider"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.surface = crate::ui::shell::Surface::Management;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let mut cards = v_flex().gap_3();
        let reorderable = snapshot.accounts.len() > 1;
        for acc in &snapshot.accounts {
            cards =
                cards.child(self.render_account_card(acc.clone(), reorderable, &theme, &lang, cx));
        }

        v_flex()
            .gap_4()
            .child(toolbar)
            .child(hero)
            .child(empty)
            .child(cards)
            .into_any_element()
    }

    fn render_account_card(
        &self,
        acc: AccountDashboard,
        reorderable: bool,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_error =
            acc.credential_status != crate::domain::CredentialStatus::Valid || acc.error.is_some();
        let w = worst_tier(&acc.tiers).cloned();
        let border_color = match &w {
            Some(t) => status_color(theme, t.forecast.state),
            None => theme.colors.border,
        };

        let status_badge = if has_error {
            let label = match acc.credential_status {
                crate::domain::CredentialStatus::Expired => {
                    crate::i18n::tr(lang, "status.credentialExpired")
                }
                crate::domain::CredentialStatus::Unavailable => {
                    crate::i18n::tr(lang, "status.credentialUnavailable")
                }
                _ => crate::i18n::tr(lang, "status.credentialMisconfigured"),
            };
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .bg(theme.colors.danger.opacity(0.12))
                .text_color(theme.colors.danger)
                .child(label)
                .into_any_element()
        } else if let Some(t) = &w {
            div()
                .px_2()
                .py_1()
                .rounded_md()
                .text_xs()
                .border_1()
                .border_color(status_color(theme, t.forecast.state).opacity(0.3))
                .bg(status_color(theme, t.forecast.state).opacity(0.1))
                .text_color(status_color(theme, t.forecast.state))
                .child(status_text(lang, t.forecast.state))
                .into_any_element()
        } else {
            div().into_any_element()
        };

        let body = if has_error {
            div()
                .pl_6()
                .text_xs()
                .text_color(theme.colors.danger.opacity(0.9))
                .child(
                    acc.error
                        .clone()
                        .unwrap_or_else(|| crate::i18n::tr(lang, "overview.credentialError")),
                )
                .into_any_element()
        } else if !acc.tiers.is_empty() {
            let mut col = v_flex().pl_6().gap_2();
            for tier in sorted_tiers(&acc.tiers) {
                let tier_id = tier.quota.id.clone();
                let account_id = acc.account.id.clone();
                let content = tier_content(theme, tier, lang, false);
                col = col.child(
                    div()
                        .id(format!("tier-{account_id}-{tier_id}"))
                        .rounded_md()
                        .border_1()
                        .p_2()
                        .cursor_pointer()
                        .border_color(theme.colors.border.opacity(0.6))
                        .hover(|s| s.bg(theme.colors.background.opacity(0.5)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_tier_detail(account_id.clone(), tier_id.clone(), window, cx);
                        }))
                        .child(content),
                );
            }
            col.into_any_element()
        } else {
            div()
                .pl_6()
                .text_xs()
                .text_color(theme.colors.muted_foreground)
                .child(crate::i18n::tr(lang, "overview.noQuota"))
                .into_any_element()
        };

        v_flex()
            .gap_3()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(border_color.opacity(0.45))
            .bg(theme.colors.background.opacity(0.35))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .gap_2()
                    .items_start()
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(acc.account.display_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!("{:?}", acc.account.provider).to_lowercase()),
                            ),
                    )
                    .child(status_badge)
                    .when(reorderable, |row| {
                        let name = acc.account.display_name.clone();
                        let up_id = acc.account.id.clone();
                        let down_id = acc.account.id.clone();
                        row.child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new(format!("move-up-{up_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label("↑")
                                        .tooltip(crate::i18n::tr_params(
                                            lang,
                                            "overview.moveUp",
                                            &[("name", &name)],
                                        ))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            move_account(this, &up_id, -1, cx);
                                        })),
                                )
                                .child(
                                    Button::new(format!("move-down-{down_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label("↓")
                                        .tooltip(crate::i18n::tr_params(
                                            lang,
                                            "overview.moveDown",
                                            &[("name", &name)],
                                        ))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            move_account(this, &down_id, 1, cx);
                                        })),
                                ),
                        )
                    }),
            )
            .child(body)
            .into_any_element()
    }

    fn open_tier_detail(
        &mut self,
        account_id: String,
        tier_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let Some(account) = self
            .store
            .read(cx)
            .snapshot
            .as_ref()
            .and_then(|s| s.accounts.iter().find(|a| a.account.id == account_id))
            .cloned()
        else {
            return;
        };

        let detail = cx.new(|cx| TierDetail::new(store, account, tier_id, cx));
        let lang = self.lang(cx);
        let title = format!(
            "{} · {}",
            detail.read(cx).account.account.display_name,
            detail.read(cx).selected_tier
        );
        let description = crate::i18n::tr(&lang, "overview.dialogDesc");
        window.open_dialog(cx, move |dialog, _, cx| {
            dialog
                .title(div().child(title.clone()))
                .child(detail.clone())
                .footer(
                    Button::new("detail-close")
                        .label(crate::i18n::tr(&lang, "providers.form.cancel"))
                        .on_click(|_, window, cx| {
                            window.close_dialog(cx);
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().colors.muted_foreground)
                        .child(description.clone()),
                )
        });
    }
}
