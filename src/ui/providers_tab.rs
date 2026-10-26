//! Providers tab: one-click import, CLI discovery, manual accounts, and the
//! add/edit account dialog form.

use std::collections::HashMap;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::form::Field;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::WindowExt;
use gpui_kit::component::{h_flex, v_flex, Disableable, Sizable};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::domain::{
    AccountInput, AlertRule, CredentialStatus, ProviderConfig, ProviderKind, PublicAccount,
    SecretPayload,
};
use crate::ui::shell::AppShell;
use crate::ui::store::{Busy, DeviceFlowStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMode {
    Add,
    Edit,
    AlertsOnly,
}

#[derive(Debug, Clone)]
pub struct AlertMeta {
    pub label: String,
    pub unlimited: bool,
    pub missing: bool,
}

/// State for the add/edit account dialog. Text fields are `InputState`
/// entities so GPUI owns focus and editing behavior.
pub struct AccountForm {
    pub mode: FormMode,
    pub editing: Option<PublicAccount>,
    pub provider: ProviderKind,
    pub enabled: bool,
    pub show_secret: bool,
    pub github_domain: String,
    pub zhipu_region: String,
    pub minimax_region: String,
    pub volc_region: String,
    pub in_name: Entity<InputState>,
    pub in_github: Entity<InputState>,
    pub in_zhipu_org: Entity<InputState>,
    pub in_zhipu_proj: Entity<InputState>,
    pub in_zenmux_url: Entity<InputState>,
    pub in_volc_region: Entity<InputState>,
    pub in_api_key: Entity<InputState>,
    pub in_access_key: Entity<InputState>,
    pub in_secret_key: Entity<InputState>,
    pub alert_drafts: Vec<(AlertRule, Entity<InputState>)>,
    pub alert_meta: HashMap<String, AlertMeta>,
}

const PROVIDER_CHOICES: [ProviderKind; 7] = [
    ProviderKind::Kimi,
    ProviderKind::Zhipu,
    ProviderKind::ZhipuTeam,
    ProviderKind::Minimax,
    ProviderKind::Zenmux,
    ProviderKind::Volcengine,
    ProviderKind::Copilot,
];

fn provider_i18n_key(p: ProviderKind) -> &'static str {
    match p {
        ProviderKind::Claude => "claude",
        ProviderKind::Codex => "codex",
        ProviderKind::Gemini => "gemini",
        ProviderKind::Copilot => "copilot",
        ProviderKind::Kimi => "kimi",
        ProviderKind::Zhipu => "zhipu",
        ProviderKind::ZhipuTeam => "zhipu_team",
        ProviderKind::Minimax => "minimax",
        ProviderKind::Zenmux => "zenmux",
        ProviderKind::Volcengine => "volcengine",
    }
}

impl AccountForm {
    fn input(
        window: &mut Window,
        cx: &mut App,
        value: &str,
        placeholder: &str,
    ) -> Entity<InputState> {
        cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(placeholder);
            if !value.is_empty() {
                state.set_value(value, window, cx);
            }
            state
        })
    }

    pub fn add(window: &mut Window, cx: &mut App) -> Self {
        Self {
            mode: FormMode::Add,
            editing: None,
            provider: ProviderKind::Kimi,
            enabled: true,
            show_secret: false,
            github_domain: String::new(),
            zhipu_region: "cn".into(),
            minimax_region: "cn".into(),
            volc_region: "cn-beijing".into(),
            in_name: Self::input(window, cx, "", ""),
            in_github: Self::input(window, cx, "", ""),
            in_zhipu_org: Self::input(window, cx, "", ""),
            in_zhipu_proj: Self::input(window, cx, "", ""),
            in_zenmux_url: Self::input(window, cx, "", ""),
            in_volc_region: Self::input(window, cx, "cn-beijing", ""),
            in_api_key: Self::input(window, cx, "", ""),
            in_access_key: Self::input(window, cx, "", ""),
            in_secret_key: Self::input(window, cx, "", ""),
            alert_drafts: Vec::new(),
            alert_meta: HashMap::new(),
        }
    }

    pub fn edit(acc: &PublicAccount, alerts_only: bool, window: &mut Window, cx: &mut App) -> Self {
        let mut form = Self {
            mode: if alerts_only {
                FormMode::AlertsOnly
            } else {
                FormMode::Edit
            },
            editing: Some(acc.clone()),
            provider: acc.provider,
            enabled: acc.enabled,
            show_secret: false,
            github_domain: String::new(),
            zhipu_region: "cn".into(),
            minimax_region: "cn".into(),
            volc_region: "cn-beijing".into(),
            in_name: Self::input(window, cx, &acc.display_name, ""),
            in_github: Self::input(window, cx, "", ""),
            in_zhipu_org: Self::input(window, cx, "", ""),
            in_zhipu_proj: Self::input(window, cx, "", ""),
            in_zenmux_url: Self::input(window, cx, "", ""),
            in_volc_region: Self::input(window, cx, "cn-beijing", ""),
            in_api_key: Self::input(window, cx, "", ""),
            in_access_key: Self::input(window, cx, "", ""),
            in_secret_key: Self::input(window, cx, "", ""),
            alert_drafts: Vec::new(),
            alert_meta: HashMap::new(),
        };
        match &acc.config {
            ProviderConfig::Copilot { github_domain } => {
                form.github_domain = github_domain.clone().unwrap_or_default();
                form.in_github = Self::input(window, cx, &form.github_domain, "");
            }
            ProviderConfig::Zhipu { region } => form.zhipu_region = region.clone(),
            ProviderConfig::ZhipuTeam {
                organization_id,
                project_id,
            } => {
                form.in_zhipu_org = Self::input(window, cx, organization_id, "");
                form.in_zhipu_proj = Self::input(window, cx, project_id, "");
            }
            ProviderConfig::Minimax { region } => form.minimax_region = region.clone(),
            ProviderConfig::Zenmux { quota_url } => {
                form.in_zenmux_url = Self::input(window, cx, quota_url, "");
            }
            ProviderConfig::Volcengine { region } => {
                form.volc_region = region.clone();
                form.in_volc_region = Self::input(window, cx, region, "");
            }
            _ => {}
        }
        form
    }

    /// Build alert-rule drafts from the account's current rules plus the
    /// tiers observed in the latest dashboard snapshot.
    pub fn build_alert_drafts(
        &mut self,
        acc: &PublicAccount,
        tiers: Vec<(String, String, bool)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let by_id: HashMap<String, &AlertRule> = acc
            .alert_rules
            .iter()
            .map(|r| (r.tier_id.clone(), r))
            .collect();
        let mut drafts = Vec::new();
        for (tier_id, label, unlimited) in tiers {
            let existing = by_id.get(&tier_id).copied();
            let threshold = existing.map(|r| r.threshold_percent).unwrap_or(80);
            self.alert_meta.insert(
                tier_id.clone(),
                AlertMeta {
                    label,
                    unlimited,
                    missing: false,
                },
            );
            drafts.push((
                AlertRule {
                    tier_id,
                    enabled: if unlimited {
                        false
                    } else {
                        existing.map(|r| r.enabled).unwrap_or(false)
                    },
                    threshold_percent: threshold,
                },
                Self::input(window, cx, &threshold.to_string(), ""),
            ));
        }
        // Rules whose tier vanished upstream stay editable but flagged.
        for (id, rule) in by_id {
            if self.alert_meta.contains_key(&id) {
                continue;
            }
            self.alert_meta.insert(
                id.clone(),
                AlertMeta {
                    label: id.clone(),
                    unlimited: false,
                    missing: true,
                },
            );
            drafts.push((
                rule.clone(),
                Self::input(window, cx, &rule.threshold_percent.to_string(), ""),
            ));
        }
        self.alert_drafts = drafts;
    }
}

impl AppShell {
    pub fn render_providers(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let lang = self.lang(cx);
        let (accounts, probes, busy) = {
            let store = self.store.read(cx);
            (store.accounts.clone(), store.probes.clone(), store.busy)
        };

        // ── One-click import ──
        let import_card = h_flex()
            .w_full()
            .p_3()
            .justify_between()
            .items_center()
            .rounded_lg()
            .border_1()
            .border_color(theme.colors.border.opacity(0.7))
            .bg(theme.colors.background.opacity(0.35))
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(crate::i18n::tr(&lang, "providers.oneClickImport.title")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "providers.oneClickImport.desc")),
                    ),
            )
            .child(
                Button::new("one-click-import")
                    .label(crate::i18n::tr(&lang, "providers.oneClickImport.button"))
                    .loading(busy == Busy::Importing)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.one_click_import(cx));
                    })),
            );

        // ── CLI discovery ──
        let mut cli_section = v_flex().gap_2().child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "providers.cli.section")),
                )
                .child(
                    Button::new("rescan-cli")
                        .ghost()
                        .small()
                        .label(crate::i18n::tr(&lang, "providers.cli.rescan"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, |store, cx| store.reload_all(cx));
                        })),
                ),
        );
        for probe in &probes {
            let name_key = match probe.provider {
                ProviderKind::Claude => "providers.cli.claudeName",
                ProviderKind::Codex => "providers.cli.codexName",
                _ => "providers.cli.geminiName",
            };
            let guide_key = match probe.provider {
                ProviderKind::Claude => "providers.cli.claudeGuide",
                ProviderKind::Codex => "providers.cli.codexGuide",
                _ => "providers.cli.geminiGuide",
            };
            let status_label = match probe.status {
                CredentialStatus::Valid => crate::i18n::tr(&lang, "providers.cli.detected"),
                CredentialStatus::Expired => crate::i18n::tr(&lang, "providers.cli.expired"),
                CredentialStatus::NotFound => crate::i18n::tr(&lang, "providers.cli.notFound"),
                _ => crate::i18n::tr(&lang, "providers.cli.error"),
            };
            let status_color = match probe.status {
                CredentialStatus::Valid => theme.colors.success,
                CredentialStatus::Expired => theme.colors.warning,
                _ => theme.colors.muted_foreground,
            };
            cli_section = cli_section.child(
                h_flex()
                    .w_full()
                    .p_3()
                    .justify_between()
                    .items_start()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.colors.border.opacity(0.6))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(crate::i18n::tr(&lang, name_key)),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .rounded_md()
                                            .text_xs()
                                            .border_1()
                                            .border_color(status_color.opacity(0.25))
                                            .text_color(status_color)
                                            .child(status_label),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(crate::i18n::tr(&lang, guide_key)),
                            ),
                    )
                    .when(
                        probe.status == CredentialStatus::Valid
                            && accounts
                                .iter()
                                .any(|a| Some(&a.id) == probe.account_id.as_ref()),
                        |row| {
                            let acc_id = probe.account_id.clone().unwrap_or_default();
                            row.child(
                                Button::new(format!("alerts-{acc_id}"))
                                    .outline()
                                    .small()
                                    .label(crate::i18n::tr(&lang, "providers.alertSettingsBtn"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let acc = this
                                            .store
                                            .read(cx)
                                            .accounts
                                            .iter()
                                            .find(|a| a.id == acc_id);
                                        if let Some(acc) = acc.cloned() {
                                            this.open_account_form(Some(acc), true, window, cx);
                                        }
                                    })),
                            )
                        },
                    ),
            );
        }

        // ── Manual accounts ──
        let manual: Vec<PublicAccount> = accounts
            .iter()
            .filter(|a| a.credential_source != crate::domain::CredentialSource::CliAuto)
            .cloned()
            .collect();

        let mut manual_list = v_flex().gap_2();
        for acc in &manual {
            let edit_id = acc.id.clone();
            let delete_id = acc.id.clone();
            manual_list = manual_list.child(
                h_flex()
                    .w_full()
                    .p_3()
                    .justify_between()
                    .items_center()
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.colors.border.opacity(0.6))
                    .bg(theme.colors.background.opacity(0.3))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(acc.display_name.clone()),
                                    )
                                    .when(!acc.enabled, |row| {
                                        row.child(
                                            div()
                                                .px_1()
                                                .rounded_md()
                                                .text_xs()
                                                .text_color(theme.colors.muted_foreground)
                                                .child(crate::i18n::tr(
                                                    &lang,
                                                    "providers.manual.disabled",
                                                )),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.muted_foreground)
                                    .child(format!(
                                        "{} · {}",
                                        crate::i18n::tr(
                                            &lang,
                                            &format!(
                                                "providers.provider.{}",
                                                provider_i18n_key(acc.provider)
                                            )
                                        ),
                                        if matches!(acc.config, ProviderConfig::Volcengine { .. }) {
                                            crate::i18n::tr(&lang, "providers.manual.sourceVolc")
                                        } else {
                                            crate::i18n::tr(&lang, "providers.manual.sourceApi")
                                        }
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new(format!("edit-{edit_id}"))
                                    .ghost()
                                    .label(crate::i18n::tr(&lang, "providers.manual.edit"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let acc = this
                                            .store
                                            .read(cx)
                                            .accounts
                                            .iter()
                                            .find(|a| a.id == edit_id)
                                            .cloned();
                                        if let Some(acc) = acc {
                                            this.open_account_form(Some(acc), false, window, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new(format!("delete-{delete_id}"))
                                    .ghost()
                                    .label(crate::i18n::tr(&lang, "providers.manual.delete"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let acc = this
                                            .store
                                            .read(cx)
                                            .accounts
                                            .iter()
                                            .find(|a| a.id == delete_id)
                                            .cloned();
                                        if let Some(acc) = acc {
                                            this.confirm_delete(acc, window, cx);
                                        }
                                    })),
                            ),
                    ),
            );
        }

        let empty = if manual.is_empty() {
            v_flex()
                .gap_1()
                .p_6()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(crate::i18n::tr(&lang, "providers.manual.emptyTitle")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(&lang, "providers.manual.emptyDesc")),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        v_flex()
            .gap_4()
            .child(import_card)
            .child(cli_section)
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.colors.muted_foreground)
                            .child(crate::i18n::tr(&lang, "providers.manual.section")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("auto-discover")
                                    .outline()
                                    .small()
                                    .label(crate::i18n::tr(&lang, "providers.manual.autoDiscover"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.store.update(cx, |store, cx| store.discover_env(cx));
                                    })),
                            )
                            .child(
                                Button::new("add-account")
                                    .label(crate::i18n::tr(&lang, "providers.manual.add"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_account_form(None, false, window, cx);
                                    })),
                            ),
                    ),
            )
            .child(empty)
            .child(manual_list)
            .into_any_element()
    }

    pub fn open_account_form(
        &mut self,
        account: Option<PublicAccount>,
        alerts_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let form = match &account {
            Some(acc) => AccountForm::edit(acc, alerts_only, window, cx),
            None => AccountForm::add(window, cx),
        };
        // Alert drafts come from the dashboard's observed tiers.
        let mut form = form;
        if let Some(acc) = &account {
            let tiers: Vec<(String, String, bool)> = self
                .store
                .read(cx)
                .snapshot
                .as_ref()
                .and_then(|s| s.accounts.iter().find(|a| a.account.id == acc.id))
                .map(|a| {
                    a.tiers
                        .iter()
                        .map(|t| (t.quota.id.clone(), t.quota.label.clone(), t.quota.unlimited))
                        .collect()
                })
                .unwrap_or_default();
            form.build_alert_drafts(acc, tiers, window, cx);
        }
        self.form = Some(form);
        cx.notify();
    }

    pub fn confirm_delete(
        &mut self,
        account: PublicAccount,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let lang = self.lang(cx);
        let name = account.display_name.clone();
        let account_id = account.id.clone();
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let account_id = account_id.clone();
            let store = store.clone();
            alert
                .title(crate::i18n::tr(&lang, "providers.delete.title"))
                .description(crate::i18n::tr_params(
                    &lang,
                    "providers.delete.desc",
                    &[("name", &name)],
                ))
                .show_cancel(true)
                .ok_text(crate::i18n::tr(&lang, "providers.delete.confirm"))
                .on_ok(move |_, _, cx| {
                    store.update(cx, |store, cx| store.delete_account(account_id.clone(), cx));
                    true
                })
                .on_cancel(|_, _, _| true)
        });
        self.deleting = Some(account);
    }
}

/// Form dialog rendering lives on `AppShell` but reads `AccountForm` state.
impl AppShell {
    pub fn render_account_form(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(form) = &self.form else {
            return div().into_any_element();
        };
        let theme = cx.theme().clone();
        let lang = self.lang(cx);

        let provider_ix = PROVIDER_CHOICES
            .iter()
            .position(|p| *p == form.provider)
            .unwrap_or(0);

        let provider_radios = RadioGroup::new("provider-kind")
            .children(
                PROVIDER_CHOICES
                    .iter()
                    .map(|p| {
                        Radio::new(format!("provider-{}", provider_i18n_key(*p))).label(
                            crate::i18n::tr(
                                &lang,
                                &format!("providers.provider.{}", provider_i18n_key(*p)),
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .selected_index(Some(provider_ix))
            .on_change(cx.listener(|this, ix, _, cx| {
                if let Some(form) = &mut this.form {
                    form.provider = PROVIDER_CHOICES[*ix];
                    cx.notify();
                }
            }));

        let mut fields = v_flex().gap_3();
        if form.mode != FormMode::AlertsOnly {
            fields = fields
                .child(
                    Field::new()
                        .label(crate::i18n::tr(&lang, "providers.form.type"))
                        .child(provider_radios),
                )
                .child(
                    Field::new()
                        .label(crate::i18n::tr(&lang, "providers.form.name"))
                        .child(Input::new(&form.in_name)),
                );

            match form.provider {
                ProviderKind::Copilot => {
                    fields = fields.child(
                        Field::new()
                            .label(crate::i18n::tr(&lang, "providers.form.githubDomain"))
                            .description(crate::i18n::tr(&lang, "providers.form.githubDomainDesc"))
                            .child(Input::new(&form.in_github)),
                    );
                    fields = fields.child(self.render_device_flow(&theme, &lang, cx));
                }
                _ => {
                    if form.provider == ProviderKind::Zhipu {
                        let region_ix = if form.zhipu_region == "global" { 1 } else { 0 };
                        fields = fields.child(
                            Field::new()
                                .label(crate::i18n::tr(&lang, "providers.form.region"))
                                .child(
                                    RadioGroup::new("zhipu-region")
                                        .children([
                                            Radio::new("zhipu-cn").label(crate::i18n::tr(
                                                &lang,
                                                "providers.form.zhipuRegionCn",
                                            )),
                                            Radio::new("zhipu-global").label(crate::i18n::tr(
                                                &lang,
                                                "providers.form.zhipuRegionGlobal",
                                            )),
                                        ])
                                        .selected_index(Some(region_ix))
                                        .on_change(cx.listener(|this, ix, _, cx| {
                                            if let Some(form) = &mut this.form {
                                                form.zhipu_region = if *ix == 1 {
                                                    "global".into()
                                                } else {
                                                    "cn".into()
                                                };
                                                cx.notify();
                                            }
                                        })),
                                ),
                        );
                    }
                    if form.provider == ProviderKind::ZhipuTeam {
                        fields = fields
                            .child(
                                Field::new()
                                    .label(crate::i18n::tr(&lang, "providers.form.orgId"))
                                    .child(Input::new(&form.in_zhipu_org)),
                            )
                            .child(
                                Field::new()
                                    .label(crate::i18n::tr(&lang, "providers.form.projectId"))
                                    .child(Input::new(&form.in_zhipu_proj)),
                            );
                    }
                    if form.provider == ProviderKind::Minimax {
                        let region_ix = if form.minimax_region == "global" {
                            1
                        } else {
                            0
                        };
                        fields = fields.child(
                            Field::new()
                                .label(crate::i18n::tr(&lang, "providers.form.minimaxRegion"))
                                .child(
                                    RadioGroup::new("minimax-region")
                                        .children([
                                            Radio::new("mm-cn").label(crate::i18n::tr(
                                                &lang,
                                                "providers.form.minimaxRegionCn",
                                            )),
                                            Radio::new("mm-global").label(crate::i18n::tr(
                                                &lang,
                                                "providers.form.minimaxRegionGlobal",
                                            )),
                                        ])
                                        .selected_index(Some(region_ix))
                                        .on_change(cx.listener(|this, ix, _, cx| {
                                            if let Some(form) = &mut this.form {
                                                form.minimax_region = if *ix == 1 {
                                                    "global".into()
                                                } else {
                                                    "cn".into()
                                                };
                                                cx.notify();
                                            }
                                        })),
                                ),
                        );
                    }
                    if form.provider == ProviderKind::Zenmux {
                        fields = fields.child(
                            Field::new()
                                .label(crate::i18n::tr(&lang, "providers.form.zenmuxUrl"))
                                .description(crate::i18n::tr(&lang, "providers.form.zenmuxUrlDesc"))
                                .child(Input::new(&form.in_zenmux_url)),
                        );
                    }
                    if form.provider == ProviderKind::Volcengine {
                        fields = fields
                            .child(
                                Field::new()
                                    .label(crate::i18n::tr(
                                        &lang,
                                        "providers.form.volcengineRegion",
                                    ))
                                    .child(Input::new(&form.in_volc_region)),
                            )
                            .child(
                                Field::new()
                                    .label(crate::i18n::tr(&lang, "providers.form.accessKeyId"))
                                    .child(Input::new(&form.in_access_key)),
                            );
                    }

                    // Secret field: AK/SK pair for Volcengine, API key otherwise.
                    let secret_field = if form.provider == ProviderKind::Volcengine {
                        Field::new()
                            .label(crate::i18n::tr(&lang, "providers.form.secretAccessKey"))
                            .child(Input::new(&form.in_secret_key).mask_toggle())
                    } else {
                        Field::new()
                            .label(crate::i18n::tr(&lang, "providers.form.apiKey"))
                            .child(Input::new(&form.in_api_key).mask_toggle())
                    };
                    fields = fields.child(secret_field);
                }
            }
        }

        // ── Alert rules ──
        let mut alert_fields = v_flex().gap_2().child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(crate::i18n::tr(&lang, "providers.alert.sectionTitle")),
        );
        if form.alert_drafts.is_empty() {
            alert_fields = alert_fields.child(
                div()
                    .text_xs()
                    .text_color(theme.colors.muted_foreground)
                    .child(crate::i18n::tr(&lang, "providers.alert.afterFirstRefresh")),
            );
        }
        for (rule, input) in &form.alert_drafts {
            let meta = form.alert_meta.get(&rule.tier_id);
            let unlimited = meta.map(|m| m.unlimited).unwrap_or(false);
            let label = meta
                .map(|m| m.label.clone())
                .unwrap_or_else(|| rule.tier_id.clone());
            let tier_id = rule.tier_id.clone();
            alert_fields = alert_fields.child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.colors.border.opacity(0.5))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex().gap_1().child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(label.clone()),
                                ),
                            )
                            .child(
                                Switch::new(format!("alert-toggle-{tier_id}"))
                                    .checked(rule.enabled && !unlimited)
                                    .disabled(unlimited)
                                    .on_change(cx.listener(move |this, enabled, _, cx| {
                                        if let Some(form) = &mut this.form {
                                            if let Some(draft) = form
                                                .alert_drafts
                                                .iter_mut()
                                                .find(|(r, _)| r.tier_id == tier_id)
                                            {
                                                draft.0.enabled = *enabled;
                                            }
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .when(!unlimited, |col| {
                        col.child(
                            Field::new()
                                .label(crate::i18n::tr(&lang, "providers.alert.thresholdLabel"))
                                .child(Input::new(input)),
                        )
                    }),
            );
        }
        fields = fields.child(alert_fields);

        if form.mode != FormMode::AlertsOnly {
            let enabled = form.enabled;
            fields =
                fields.child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(crate::i18n::tr(&lang, "providers.form.enabled")),
                        )
                        .child(Switch::new("form-enabled").checked(enabled).on_change(
                            cx.listener(|this, value, _, cx| {
                                if let Some(form) = &mut this.form {
                                    form.enabled = *value;
                                }
                                cx.notify();
                            }),
                        )),
                );
        }

        let submit = Button::new("form-submit")
            .primary()
            .label(crate::i18n::tr(&lang, "providers.form.confirm"))
            .on_click(cx.listener(|this, _, window, cx| {
                this.submit_account_form(window, cx);
            }));

        v_flex()
            .gap_3()
            .child(fields)
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        Button::new("form-cancel")
                            .outline()
                            .label(crate::i18n::tr(&lang, "providers.form.cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.form = None;
                                window.close_dialog(cx);
                                cx.notify();
                            })),
                    )
                    .child(submit),
            )
            .into_any_element()
    }

    fn render_device_flow(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        lang: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let flow = self.store.read(cx).device_flow.clone();
        match flow {
            None => Button::new("start-device-flow")
                .label(crate::i18n::tr(lang, "providers.form.startLink"))
                .on_click(cx.listener(|this, _, _, cx| {
                    let domain = this
                        .form
                        .as_ref()
                        .map(|f| f.github_domain.trim().to_string())
                        .filter(|s| !s.is_empty());
                    this.store.update(cx, |store, cx| {
                        store.start_device_flow(domain, cx);
                    });
                }))
                .into_any_element(),
            Some(DeviceFlowStatus::Requesting) => div()
                .text_xs()
                .text_color(theme.colors.muted_foreground)
                .child(crate::i18n::tr(lang, "providers.form.requesting"))
                .into_any_element(),
            Some(DeviceFlowStatus::Failed(msg)) => div()
                .text_xs()
                .text_color(theme.colors.danger)
                .child(msg)
                .into_any_element(),
            Some(DeviceFlowStatus::Polling {
                user_code,
                verify_uri,
                ..
            }) => v_flex()
                .gap_2()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(theme.colors.border)
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(crate::i18n::tr(lang, "providers.form.deviceCode")),
                )
                .child(
                    div()
                        .p_2()
                        .rounded_md()
                        .font_family(theme.mono_font_family.clone())
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.colors.foreground)
                        .child(user_code),
                )
                .child(
                    h_flex().gap_2().child(
                        Button::new("open-verify")
                            .outline()
                            .small()
                            .label(crate::i18n::tr(lang, "providers.form.openBrowser"))
                            .on_click(move |_, _, cx| {
                                cx.open_url(&verify_uri);
                            }),
                    ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.muted_foreground)
                        .child(crate::i18n::tr(lang, "providers.form.waiting")),
                )
                .into_any_element(),
        }
    }

    pub fn submit_account_form(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let lang = self.lang(cx);

        let mut alert_rules = Vec::new();
        let mut invalid_threshold = false;
        for (rule, input) in &form.alert_drafts {
            let raw = input.read(cx).value().to_string();
            let threshold: u8 = raw.trim().parse().unwrap_or(0);
            let meta = form.alert_meta.get(&rule.tier_id);
            let unlimited = meta.map(|m| m.unlimited).unwrap_or(false);
            if !unlimited && !(1..=99).contains(&threshold) {
                invalid_threshold = true;
            }
            alert_rules.push(AlertRule {
                tier_id: rule.tier_id.clone(),
                enabled: rule.enabled && !unlimited,
                threshold_percent: if unlimited {
                    threshold.clamp(1, 99)
                } else {
                    threshold
                },
            });
        }
        if invalid_threshold {
            self.store.update(cx, |store, cx| {
                store.toast(store.t("providers.toasts.thresholdInvalid"), true);
                cx.notify();
            });
            return;
        }

        if form.mode == FormMode::AlertsOnly {
            if let Some(acc) = form.editing.clone() {
                let input = AccountInput {
                    id: Some(acc.id),
                    display_name: acc.display_name,
                    enabled: acc.enabled,
                    config: acc.config,
                    secret: None,
                    remove_credential: None,
                    alert_rules,
                };
                self.form = None;
                self.store
                    .update(cx, |store, cx| store.save_account(input, cx));
                cx.notify();
            }
            return;
        }

        let display_name = form.in_name.read(cx).value().to_string();
        if display_name.trim().is_empty() {
            self.store.update(cx, |store, cx| {
                store.toast(store.t("providers.toasts.nameRequired"), true);
                cx.notify();
            });
            return;
        }

        let provider = form.provider;
        let config = match provider {
            ProviderKind::Copilot => ProviderConfig::Copilot {
                github_domain: {
                    let v = form.in_github.read(cx).value().to_string();
                    let v = v.trim().to_string();
                    if v.is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                },
            },
            ProviderKind::Zhipu => ProviderConfig::Zhipu {
                region: form.zhipu_region.clone(),
            },
            ProviderKind::ZhipuTeam => {
                let org = form.in_zhipu_org.read(cx).value().to_string();
                let proj = form.in_zhipu_proj.read(cx).value().to_string();
                if org.trim().is_empty() || proj.trim().is_empty() {
                    self.store.update(cx, |store, cx| {
                        store.toast(store.t("providers.toasts.zhipuTeamIdRequired"), true);
                        cx.notify();
                    });
                    return;
                }
                ProviderConfig::ZhipuTeam {
                    organization_id: org.trim().to_string(),
                    project_id: proj.trim().to_string(),
                }
            }
            ProviderKind::Minimax => ProviderConfig::Minimax {
                region: form.minimax_region.clone(),
            },
            ProviderKind::Zenmux => {
                let url = form.in_zenmux_url.read(cx).value().to_string();
                let url = url.trim().to_string();
                if !url.starts_with("https://") {
                    self.store.update(cx, |store, cx| {
                        store.toast(store.t("providers.toasts.zenmuxHttps"), true);
                        cx.notify();
                    });
                    return;
                }
                if !url.contains("zenmux.") {
                    self.store.update(cx, |store, cx| {
                        store.toast(store.t("providers.toasts.zenmuxDomain"), true);
                        cx.notify();
                    });
                    return;
                }
                ProviderConfig::Zenmux { quota_url: url }
            }
            ProviderKind::Volcengine => ProviderConfig::Volcengine {
                region: {
                    let v = form.in_volc_region.read(cx).value().to_string();
                    let v = v.trim().to_string();
                    if v.is_empty() {
                        "cn-beijing".into()
                    } else {
                        v
                    }
                },
            },
            ProviderKind::Claude => ProviderConfig::Claude,
            ProviderKind::Codex => ProviderConfig::Codex,
            ProviderKind::Gemini => ProviderConfig::Gemini,
            ProviderKind::Kimi => ProviderConfig::Kimi,
        };

        // Secret: empty means "keep existing".
        let secret = if provider == ProviderKind::Volcengine {
            let ak = form.in_access_key.read(cx).value().to_string();
            let sk = form.in_secret_key.read(cx).value().to_string();
            if !ak.trim().is_empty() && !sk.trim().is_empty() {
                Some(SecretPayload::Volcengine {
                    access_key_id: ak,
                    secret_access_key: sk,
                })
            } else {
                None
            }
        } else if provider != ProviderKind::Copilot {
            let key = form.in_api_key.read(cx).value().to_string();
            if !key.trim().is_empty() {
                Some(SecretPayload::ApiKey { api_key: key })
            } else {
                None
            }
        } else {
            None
        };

        let input = AccountInput {
            id: form.editing.as_ref().map(|a| a.id.clone()),
            display_name,
            enabled: form.enabled,
            config,
            secret,
            remove_credential: None,
            alert_rules,
        };
        let _ = lang;
        self.form = None;
        self.store
            .update(cx, |store, cx| store.save_account(input, cx));
        cx.notify();
    }
}
