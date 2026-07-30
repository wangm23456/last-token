import type { DashboardSnapshot, QuotaTier, AccountDashboard, ProviderKind } from "@/types";
import type { WidgetConfig, WidgetElement } from "tauri-plugin-widgets-api";

// Widget group identifier. Used for App Group on iOS/macOS and as the
// SharedPreferences name on Android. Desktop just uses it as a filename.
export const WIDGET_GROUP = "group.com.wangmeng.last-token";

// Visual theme tokens. Stay close to the dashboard colors so the widget
// feels like an extension of the main app.
const BG = "#161821";
const SURFACE = "#1f2230";
const LABEL = "#ececf1";
const SECONDARY = "#8a8fa3";
const OK = "#22c55e";
const WARN = "#eab308";
const DANGER = "#ef4444";

// <0.5 green, 0.5-0.8 amber, >=0.8 red.
function utilizationColor(u: number): string {
  if (u >= 0.8) return DANGER;
  if (u >= 0.5) return WARN;
  return OK;
}

function pct(u: number): string {
  return `${Math.round(u * 100)}%`;
}

interface TopEntry {
  name: string;
  provider: ProviderKind;
  tier: QuotaTier;
}

// Pick the highest-utilization tier across the snapshot. Used by the
// small widget to surface the most-at-risk account.
function topTierPerAccount(accounts: AccountDashboard[]): TopEntry[] {
  const out: TopEntry[] = [];
  for (const account of accounts) {
    const tiers = account.tiers.map((t) => t.quota);
    if (tiers.length === 0) continue;
    let top = tiers[0];
    for (const t of tiers) {
      if (t.utilization > top.utilization) top = t;
    }
    out.push({
      name: account.account.displayName,
      provider: account.account.config.type,
      tier: top,
    });
  }
  return out;
}

export function buildWidgetConfig(snapshot: DashboardSnapshot): WidgetConfig {
  const accounts = snapshot.accounts.filter((a) => a.account.enabled);
  const totals = topTierPerAccount(accounts);
  const overall = totals.reduce((acc, t) => Math.max(acc, t.tier.utilization), 0);

  const headerLine = totals.length === 0
    ? "No accounts"
    : `${totals.length} account${totals.length === 1 ? "" : "s"} · max ${pct(overall)}`;

  const topAccount = totals[0];

  // Small widget: app name + most-at-risk account + bar.
  const smallChildren: WidgetElement[] = [
    { type: "text", content: "Last Token", textStyle: "caption2", color: SECONDARY },
    { type: "text", content: headerLine, textStyle: "footnote", fontWeight: "semibold", color: LABEL },
  ];
  if (topAccount) {
    smallChildren.push({ type: "divider", color: SURFACE });
    smallChildren.push({ type: "text", content: topAccount.name, textStyle: "body", color: LABEL });
    smallChildren.push({
      type: "text",
      content: `${topAccount.tier.label} · ${pct(topAccount.tier.utilization)}`,
      textStyle: "footnote",
      color: SECONDARY,
    });
    smallChildren.push({
      type: "progress",
      value: topAccount.tier.utilization,
      tint: utilizationColor(topAccount.tier.utilization),
      label: topAccount.tier.label,
      barStyle: "linear",
    });
  } else {
    smallChildren.push({
      type: "text",
      content: "Add an account to start tracking.",
      textStyle: "footnote",
      color: SECONDARY,
    });
  }
  const small = {
    type: "vstack" as const,
    children: smallChildren,
    spacing: 8,
    padding: 14,
    background: BG,
    cornerRadius: 16,
  };

  // Medium widget: each account's top tier stacked.
  const mediumChildren: WidgetElement[] = [
    {
      type: "hstack",
      children: [
        { type: "text", content: "Last Token", textStyle: "title3", fontWeight: "bold", color: LABEL },
        { type: "spacer" },
        { type: "text", content: headerLine, textStyle: "footnote", color: SECONDARY },
      ],
    },
  ];
  if (totals.length === 0) {
    mediumChildren.push({
      type: "text",
      content: "No accounts yet. Add one in the main window.",
      textStyle: "body",
      color: SECONDARY,
    });
  } else {
    for (const entry of totals) {
      const tint = utilizationColor(entry.tier.utilization);
      mediumChildren.push({
        type: "vstack",
        children: [
          {
            type: "hstack",
            children: [
              { type: "text", content: entry.name, textStyle: "body", fontWeight: "semibold", color: LABEL },
              { type: "spacer" },
              { type: "text", content: pct(entry.tier.utilization), textStyle: "body", fontWeight: "medium", color: tint },
            ],
          },
          { type: "text", content: entry.tier.label, textStyle: "caption2", color: SECONDARY },
          { type: "progress", value: entry.tier.utilization, tint: tint, barStyle: "linear" },
        ],
        spacing: 4,
        padding: 8,
        background: SURFACE,
        cornerRadius: 10,
      });
    }
  }
  const medium = {
    type: "vstack" as const,
    children: mediumChildren,
    spacing: 12,
    padding: 16,
    background: BG,
    cornerRadius: 16,
  };

  // Large widget: list every account and every tier.
  const largeChildren: WidgetElement[] = [
    {
      type: "hstack",
      children: [
        { type: "text", content: "Last Token", textStyle: "title2", fontWeight: "bold", color: LABEL },
        { type: "spacer" },
        { type: "text", content: headerLine, textStyle: "footnote", color: SECONDARY },
      ],
    },
  ];
  for (const account of accounts) {
    const tiers = account.tiers.map((t) => t.quota);
    const innerChildren: WidgetElement[] = [
      { type: "text", content: account.account.displayName, textStyle: "headline", fontWeight: "semibold", color: LABEL },
    ];
    if (tiers.length === 0) {
      innerChildren.push({
        type: "text",
        content: account.error ?? "No quota data",
        textStyle: "caption",
        color: SECONDARY,
      });
    } else {
      for (const tier of tiers) {
        const tint = utilizationColor(tier.utilization);
        innerChildren.push({
          type: "vstack",
          children: [
            {
              type: "hstack",
              children: [
                { type: "text", content: tier.label, textStyle: "subheadline", color: LABEL },
                { type: "spacer" },
                { type: "text", content: pct(tier.utilization), textStyle: "subheadline", fontWeight: "semibold", color: tint },
              ],
            },
            { type: "progress", value: tier.utilization, tint: tint, barStyle: "linear" },
          ],
          spacing: 2,
        });
      }
    }
    largeChildren.push({
      type: "vstack",
      children: innerChildren,
      spacing: tiers.length === 0 ? 4 : 6,
      padding: 10,
      background: SURFACE,
      cornerRadius: 10,
    });
  }
  const large = {
    type: "vstack" as const,
    children: largeChildren,
    spacing: 12,
    padding: 18,
    background: BG,
    cornerRadius: 16,
  };

  return { small, medium, large } as WidgetConfig;
}
