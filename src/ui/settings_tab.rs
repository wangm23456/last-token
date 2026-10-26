//! Settings tab: language, refresh interval, history management, privacy.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::WindowExt;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::Settings;
use crate::i18n::SUPPORTED_LANGUAGES;
use crate::ui::shell::AppShell;

const REFRESH_OPTIONS: [i64; 4] = [5, 10, 15, 30];

impl AppShell {
    pub fn render_settings(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let lang = self.lang(cx);
        let (settings, language) = {
            let store = self.store.read(cx);
            (store.settings.clone(), store.language.clone())
        };

        let section = |title: String, desc: Option<String>, body: AnyElement| {
            v_flex()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.colors.border.opacity(0.6))
                .bg(theme.colors.background.opacity(0.3))
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.colors.muted_foreground)
                                .child(title),
                        )
                        .when_some(desc, |col, desc| {
                            col.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(desc),
                            )
                        }),
                )
                .child(body)
        };

        let lang_ix = SUPPORTED_LANGUAGES
            .iter()
            .position(|l| *l == language)
            .unwrap_or(0);
        let language_field = v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::i18n::tr(&lang, "settings.language.label")),
            )
            .child(
                RadioGroup::new("language")
                    .children(
                        SUPPORTED_LANGUAGES
                            .iter()
                            .map(|l| {
                                Radio::new(format!("lang-{}", *l)).label(crate::i18n::tr(
                                    &lang,
                                    &format!("settings.language.options.{l}"),
                                ))
                            })
                            .collect::<Vec<_>>(),
                    )
                    .selected_index(Some(lang_ix))
                    .on_change(cx.listener(|this, ix, _, cx| {
                        let l = SUPPORTED_LANGUAGES[*ix];
                        this.store.update(cx, |store, cx| store.set_language(l, cx));
                    })),
            );

        let interval_ix = REFRESH_OPTIONS
            .iter()
            .position(|m| *m == settings.refresh_interval_minutes)
            .unwrap_or(0);
        let interval_field = v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::i18n::tr(&lang, "settings.behavior.refreshLabel")),
            )
            .child(
                RadioGroup::new("interval")
                    .children(
                        REFRESH_OPTIONS
                            .iter()
                            .map(|m| {
                                Radio::new(format!("interval-{}", *m)).label(crate::i18n::tr(
                                    &lang,
                                    &format!("settings.behavior.options.{m}"),
                                ))
                            })
                            .collect::<Vec<_>>(),
                    )
                    .selected_index(Some(interval_ix))
                    .on_change(cx.listener(|this, ix, _, cx| {
                        let mut s = this.store.read(cx).settings.clone();
                        s.refresh_interval_minutes = REFRESH_OPTIONS[*ix];
                        this.store
                            .update(cx, |store, cx| store.update_settings(s, cx));
                    })),
            );

        let data_body = v_flex()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(crate::i18n::tr(&lang, "settings.data.autoCleanupDesc")),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(crate::i18n::tr(&lang, "settings.data.clearHistory")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(crate::i18n::tr(
                                        &lang,
                                        "settings.data.clearHistoryDesc",
                                    )),
                            ),
                    )
                    .child(
                        Button::new("clear-history")
                            .danger()
                            .outline()
                            .label(crate::i18n::tr(&lang, "settings.data.clearButton"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let lang = this.lang(cx);
                                let store = this.store.clone();
                                window.open_alert_dialog(cx, move |alert, _window, _cx| {
                                    let store = store.clone();
                                    alert
                                        .title(crate::i18n::tr(&lang, "settings.data.dialogTitle"))
                                        .description(crate::i18n::tr(
                                            &lang,
                                            "settings.data.dialogDesc",
                                        ))
                                        .show_cancel(true)
                                        .ok_text(crate::i18n::tr(&lang, "settings.data.confirm"))
                                        .on_ok(move |_, _, cx| {
                                            store.update(cx, |store, cx| store.clear_history(cx));
                                            true
                                        })
                                        .on_cancel(|_, _, _| true)
                                });
                            })),
                    ),
            );

        let security_body = v_flex()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crate::i18n::tr(&lang, "settings.security.envRead")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "settings.security.envReadDesc")),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crate::i18n::tr(&lang, "settings.security.noRelay")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "settings.security.noRelayDesc")),
                    ),
            );

        v_flex()
            .gap_4()
            .child(section(
                crate::i18n::tr(&lang, "settings.behavior.title"),
                Some(crate::i18n::tr(&lang, "settings.behavior.desc")),
                v_flex()
                    .gap_4()
                    .child(language_field)
                    .child(interval_field)
                    .into_any_element(),
            ))
            .child(section(
                crate::i18n::tr(&lang, "settings.data.title"),
                Some(crate::i18n::tr(&lang, "settings.data.desc")),
                data_body.into_any_element(),
            ))
            .child(section(
                crate::i18n::tr(&lang, "settings.security.title"),
                None,
                security_body.into_any_element(),
            ))
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(crate::i18n::tr_params(
                        &lang,
                        "settings.version",
                        &[("version", env!("CARGO_PKG_VERSION"))],
                    )),
            )
            .into_any_element()
    }
}

/// Confirms and runs history clearing from the settings dialog.
pub fn confirm_clear_history(store: &Entity<crate::ui::store::AppStore>, cx: &mut App) {
    store.update(cx, |store, cx| store.clear_history(cx));
}

/// Placeholder to keep `Settings` import meaningful for future use.
#[allow(dead_code)]
fn _settings_type_anchor(s: &Settings) -> i64 {
    s.refresh_interval_minutes
}
