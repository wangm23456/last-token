//! Watch surface: left ranked account list, right full-tier detail.
//! Covers R1–R4 of the dashboard-first plan.

use gpui_kit::component::button::Button;
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::Sizable;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{px, *};

use crate::domain::{AccountDashboard, CredentialStatus, HistoryPoint, RiskState};
use crate::ui::shell::{AppShell, Surface};
use crate::ui::tier_view::{
    format_absolute_quota, format_time_margin, format_time_margin_short, rate_text,
    sorted_tiers, status_color, status_text, utilization_color, worst_tier,
};

fn countdown_short(lang: &str, acc: &AccountDashboard) -> Option<String> {
    let t = acc
        .tiers
        .iter()
        .filter_map(|t| t.forecast.exhaustion_at.or(t.quota.resets_at))
        .min()?;
    Some(format_time_margin_short(lang, Some(t)))
}

impl AppShell {
    pub fn render_watch(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let lang = self.lang(cx);

        let (accounts, effective_id) = {
            let store = self.store.read(cx);
            let Some(snapshot) = store.snapshot.as_ref() else {
                return self.watch_empty(&theme, &lang, cx);
            };
            let effective = store
                .selected_account
                .clone()
                .or_else(|| snapshot.accounts.first().map(|a| a.account.id.clone()));
            (snapshot.accounts.clone(), effective)
        };

        let Some(effective_id) = effective_id else {
            return self.watch_empty(&theme, &lang, cx);
        };
        let effective = accounts
            .iter()
            .find(|a| a.account.id == effective_id)
            .cloned()
            .unwrap_or_else(|| accounts[0].clone());

        let list = self.render_watch_list(&accounts, &effective_id, &theme, &lang, cx);
        let detail = self.render_watch_detail(&effective, &theme, &lang, cx);
        let toolbar = self.render_watch_toolbar(&theme, &lang, cx);

        v_flex()
            .size_full()
            .gap_3()
            .child(toolbar)
            .child(h_flex()
            .size_full()
            .gap_3()
            .child(
                v_flex()
                    .id("watch-list")
                    .w(px(260.))
                    .flex_shrink_0()
                    .h_full()
                    .gap_1()
                    .pr_1()
                    .overflow_y_scroll()
                    .children(list),
            )
            .child(
                v_flex()
                    .id("watch-detail")
                    .flex_1()
                    .min_w_0()
                    .flex_1()
                    .min_h_0()
                    .pr_1()
                    .overflow_y_scroll()
                    .child(detail),
            ))
            .into_any_element()
    }

    fn render_watch_toolbar(
        &mut self,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (refreshed_at, busy) = {
            let store = self.store.read(cx);
            (
                store.snapshot.as_ref().map(|s| s.refreshed_at),
                store.busy == crate::ui::store::Busy::Refreshing,
            )
        };
        let time_str = refreshed_at.map(|ms| {
            let dt = chrono::DateTime::from_timestamp_millis(ms)
                .unwrap_or_else(chrono::Utc::now)
                .with_timezone(&chrono::Local);
            dt.format("%H:%M:%S").to_string()
        });
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(match time_str {
                        Some(t) => crate::i18n::tr_params(
                            lang,
                            "overview.refreshedAt",
                            &[("time", t.as_str())],
                        ),
                        None => String::new(),
                    }),
            )
            .child(
                h_flex().gap_2().items_center().child(
                    Button::new("watch-refresh")
                        .outline()
                        .small()
                        .label(crate::i18n::tr(lang, "overview.refreshAria"))
                        .loading(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, |store, cx| store.refresh(cx));
                        })),
                ),
            )
            .into_any_element()
    }

    fn watch_empty(
        &mut self,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .id("watch-empty")
            .test_support()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .aria_label(crate::i18n::tr(lang, "overview.emptyTitle"))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::i18n::tr(lang, "overview.emptyTitle")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(crate::i18n::tr(lang, "overview.emptyDesc")),
            )
            .child(
                Button::new("watch-empty-add")
                    .label(crate::i18n::tr(lang, "overview.addProvider"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.surface = Surface::Management;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_watch_list(
        &mut self,
        accounts: &[AccountDashboard],
        selected_id: &str,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        accounts
            .iter()
            .map(|acc| {
                let has_error = acc.credential_status != CredentialStatus::Valid
                    || acc.error.is_some();
                let w = worst_tier(&acc.tiers).cloned();
                let dot_color = if has_error {
                    theme.colors.danger
                } else {
                    w.as_ref()
                        .map(|t| status_color(theme, t.forecast.state))
                        .unwrap_or(theme.colors.success)
                };
                let selected = acc.account.id == selected_id;

                let meta = if has_error {
                    let label = match acc.credential_status {
                        CredentialStatus::Expired => {
                            crate::i18n::tr(lang, "status.credentialExpired")
                        }
                        CredentialStatus::Unavailable => {
                            crate::i18n::tr(lang, "status.credentialUnavailable")
                        }
                        _ => crate::i18n::tr(lang, "status.credentialMisconfigured"),
                    };
                    div()
                        .text_xs()
                        .text_color(theme.colors.danger)
                        .child(label)
                        .into_any_element()
                } else if let Some(t) = &w {
                    let pct = if t.quota.unlimited {
                        crate::i18n::tr(lang, "overview.unlimitedQuota")
                    } else {
                        format!("{:.0}%", t.quota.utilization)
                    };
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_xs()
                                .truncate()
                                .text_color(theme.colors.muted_foreground)
                                .child(t.quota.label.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(utilization_color(theme, t))
                                .child(pct),
                        )
                        .into_any_element()
                } else {
                    div().into_any_element()
                };

                let countdown: AnyElement = if !has_error {
                    countdown_short(lang, acc)
                        .map(|c| {
                            div()
                                .text_xs()
                                .text_color(theme.colors.muted_foreground)
                                .child(c)
                                .into_any_element()
                        })
                        .unwrap_or_else(|| div().into_any_element())
                } else {
                    div().into_any_element()
                };

                let row_id = format!("acct-row-{}", acc.account.id);
                let select_id = acc.account.id.clone();
                div()
                    .id(row_id)
                    .test_support()
                    .w_full()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected {
                        theme.colors.border
                    } else {
                        theme.colors.border.opacity(0.5)
                    })
                    .bg(if selected {
                        theme.colors.button_secondary
                    } else {
                        theme.colors.background.opacity(0.3)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.select_account(select_id.clone(), cx));
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().size_2().rounded_full().bg(dot_color))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(acc.account.display_name.clone()),
                                    ),
                            )
                            .child(meta)
                            .child(countdown),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_watch_detail(
        &mut self,
        acc: &AccountDashboard,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_error =
            acc.credential_status != CredentialStatus::Valid || acc.error.is_some();

        let header = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                div()
                    .id("watch-detail-name")
                    .test_support()
                    .aria_label(acc.account.display_name.clone())
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .child(acc.account.display_name.clone()),
            )
            .child(match worst_tier(&acc.tiers) {
                Some(t) if !has_error => div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .border_1()
                    .border_color(status_color(theme, t.forecast.state).opacity(0.3))
                    .bg(status_color(theme, t.forecast.state).opacity(0.1))
                    .text_color(status_color(theme, t.forecast.state))
                    .child(status_text(lang, t.forecast.state))
                    .into_any_element(),
                _ => div().into_any_element(),
            });

        let body = if has_error {
            let err = acc
                .error
                .clone()
                .unwrap_or_else(|| crate::i18n::tr(lang, "overview.credentialError"));
            div()
                .id("watch-detail-error")
                .test_support()
                .aria_label(err.clone())
                .text_sm()
                .text_color(theme.colors.danger)
                .child(err)
                .into_any_element()
        } else if acc.tiers.is_empty() {
            div()
                .text_sm()
                .text_color(theme.colors.muted_foreground)
                .child(crate::i18n::tr(lang, "overview.noQuota"))
                .into_any_element()
        } else {
            let mut col = v_flex().gap_3();
            for tier in sorted_tiers(&acc.tiers) {
                col = col.child(self.render_tier_panel(acc, tier, theme, lang, cx));
            }
            col.into_any_element()
        };

        v_flex().gap_3().child(header).child(body).into_any_element()
    }

    fn render_tier_panel(
        &mut self,
        acc: &AccountDashboard,
        tier: &crate::domain::TierDashboard,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let quota = &tier.quota;
        let pct_color = utilization_color(theme, tier);

        // 24h history for this tier when the loaded set belongs to this account.
        let history: Option<Vec<HistoryPoint>> = {
            let store = self.store.read(cx);
            store
                .tier_histories
                .as_ref()
                .filter(|(acct, _)| acct == &acc.account.id)
                .and_then(|(_, map)| map.get(&quota.id).cloned())
        };

        let headline = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(quota.label.clone()),
            )
            .child(
                div()
                    .id(format!("tier-pct-{}-{}", acc.account.id, quota.id))
                    .test_support()
                    .aria_label(if quota.unlimited {
                        crate::i18n::tr(lang, "overview.unlimitedQuota")
                    } else {
                        format!("{:.1}%", quota.utilization)
                    })
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if quota.unlimited {
                        theme.colors.success
                    } else {
                        pct_color
                    })
                    .child(if quota.unlimited {
                        crate::i18n::tr(lang, "overview.unlimitedQuota")
                    } else {
                        format!("{:.1}%", quota.utilization)
                    }),
            );

        let bar = if quota.unlimited {
            div().into_any_element()
        } else {
            Progress::new(format!("watch-bar-{}", quota.id))
                .value(quota.utilization.clamp(0.0, 100.0) as f32)
                .color(pct_color)
                .into_any_element()
        };

        let sparkline = match &history {
            Some(pts) if !pts.is_empty() => LineChart::new(pts.clone())
                .x(|p: &HistoryPoint| p.sampled_at.to_string())
                .y(|p: &HistoryPoint| p.utilization)
                .stroke(pct_color)
                .interactive(false)
                .x_axis(false)
                .into_any_element(),
            _ => div().h(px(36.)).w_full().into_any_element(),
        };
        // Test registry wrapper: find() locates the chart by this id.
        let sparkline = div()
            .id(format!("tier-chart-{}-{}", acc.account.id, quota.id))
            .test_support()
            .w_full()
            .h(px(48.))
            .child(sparkline);

        let absolute = format_absolute_quota(quota);
        let meta_left = if quota.unlimited {
            crate::i18n::tr(lang, "status.safe")
        } else {
            rate_text(lang, tier)
        };
        let meta_right = match tier.forecast.state {
            RiskState::AtRisk if tier.forecast.exhaustion_at.is_some() => {
                let t = format_time_margin(lang, tier.forecast.exhaustion_at);
                crate::i18n::tr_params(lang, "format.exhaustsIn", &[("time", &t)])
            }
            _ if quota.resets_at.is_some() => {
                let t = format_time_margin(lang, quota.resets_at);
                crate::i18n::tr_params(lang, "format.resetsIn", &[("time", &t)])
            }
            RiskState::Exhausted => crate::i18n::tr(lang, "status.exhausted"),
            _ => crate::i18n::tr(lang, "format.resetUnknown"),
        };
        let meta_color = match tier.forecast.state {
            RiskState::Exhausted | RiskState::Error => theme.colors.danger,
            RiskState::AtRisk => theme.colors.warning,
            _ => theme.colors.muted_foreground,
        };

        let footer = h_flex()
            .w_full()
            .justify_between()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(match absolute {
                        Some(a) => a,
                        None => String::new(),
                    }),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(meta_color)
                            .child(meta_left),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(meta_color)
                            .child(meta_right),
                    ),
            );

        v_flex()
            .id(format!("tier-panel-{}-{}", acc.account.id, quota.id))
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.colors.border.opacity(0.6))
            .bg(theme.colors.background.opacity(0.3))
            .child(headline)
            .child(bar)
            .child(sparkline)
            .child(footer)
            .into_any_element()
    }
}
