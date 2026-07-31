import * as React from "react";
import i18n from "@/i18n";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import type { QuotaTier, RiskState, TierDashboard } from "@/types";

// ── Stable tier ordering ────────────────────────────────────────────
// Rank buckets per the contract:
//   0 = five_hour
//   1 = seven_day, weekly_limit, and any id starting with "gemini_"
//   2 = monthly, 30_day
//   3 = everything else
// Within a bucket, fall back to label localeCompare.
export function tierSortKey(id: string): [number, string] {
  const lower = id.toLowerCase();
  if (lower === "five_hour") {
    return [0, ""];
  }
  if (lower === "seven_day" || lower === "weekly_limit" || lower.startsWith("gemini_")) {
    return [1, ""];
  }
  if (lower === "monthly" || lower === "30_day") {
    return [2, ""];
  }
  return [3, ""];
}

function compareTiers(a: TierDashboard, b: TierDashboard): number {
  const [ra, ta] = tierSortKey(a.quota.id);
  const [rb, tb] = tierSortKey(b.quota.id);
  if (ra !== rb) return ra - rb;
  return ta.localeCompare(tb) || a.quota.label.localeCompare(b.quota.label);
}

export function sortTiers<T extends TierDashboard>(tiers: T[]): T[] {
  return [...tiers].sort(compareTiers);
}

// ── Risk severity ───────────────────────────────────────────────────
// exhausted > at_risk > unknown_reset > error > learning > safe
export function riskSeverity(state: RiskState): number {
  switch (state) {
    case "exhausted":
      return 5;
    case "at_risk":
      return 4;
    case "unknown_reset":
      return 3;
    case "error":
      return 2;
    case "learning":
      return 1;
    case "safe":
      return 0;
    default:
      return -1;
  }
}

function tieBreakTime(tier: TierDashboard): number {
  const t = tier.forecast.exhaustionAt ?? tier.quota.resetsAt;
  return t ?? Number.POSITIVE_INFINITY;
}

export function worstTier(tiers: TierDashboard[]): TierDashboard | null {
  if (tiers.length === 0) {
    return null;
  }
  let worst = tiers[0]!;
  for (let i = 1; i < tiers.length; i += 1) {
    const current = tiers[i]!;
    const diff = riskSeverity(current.forecast.state) - riskSeverity(worst.forecast.state);
    if (diff > 0) {
      worst = current;
      continue;
    }
    if (diff === 0) {
      if (current.quota.utilization > worst.quota.utilization) {
        worst = current;
        continue;
      }
      if (current.quota.utilization === worst.quota.utilization) {
        if (tieBreakTime(current) < tieBreakTime(worst)) {
          worst = current;
        }
      }
    }
  }
  return worst;
}

// ── Formatters ──────────────────────────────────────────────────────

export function formatTimeMargin(ms: number | null): string {
  if (ms == null) return "";
  const t = i18n.t.bind(i18n);
  const diff = ms - Date.now();
  if (diff <= 0) return t("format.zeroMinutes");
  const hours = Math.floor(diff / 3600000);
  const mins = Math.floor((diff % 3600000) / 60000);
  if (hours > 0) {
    return t("format.hoursMinutes", { h: hours, m: mins });
  }
  return t("format.minutes", { n: mins });
}

const numberFormatter = new Intl.NumberFormat("zh-CN");

export function formatAbsoluteQuota(tier: QuotaTier): string | null {
  if (tier.used == null || tier.limit == null) {
    return null;
  }
  const used = numberFormatter.format(tier.used);
  const limit = numberFormatter.format(tier.limit);
  const unit = tier.unit ?? "";
  return `${used} / ${limit}${unit ? ` ${unit}` : ""}`;
}

const STATUS_I18N_KEY: Record<RiskState, string> = {
  exhausted: "exhausted",
  at_risk: "atRisk",
  unknown_reset: "unknownReset",
  learning: "learning",
  safe: "safe",
  error: "error",
};

export function statusColorClass(state: RiskState): string {
  switch (state) {
    case "exhausted":
      return "text-status-danger bg-status-danger/10 border-status-danger/20";
    case "at_risk":
      return "text-status-warning bg-status-warning/10 border-status-warning/20";
    case "unknown_reset":
      return "text-muted-foreground bg-secondary/50 border-border";
    case "learning":
      return "text-status-stale bg-status-stale/10 border-status-stale/20";
    case "safe":
      return "text-status-safe bg-status-safe/10 border-status-safe/20";
    case "error":
      return "text-status-danger bg-status-danger/10 border-status-danger/20";
    default:
      return "text-muted-foreground bg-secondary border-border";
  }
}

export function statusAccentClass(state: RiskState | "error"): string {
  switch (state) {
    case "exhausted":
    case "error":
      return "border-l-status-danger";
    case "at_risk":
      return "border-l-status-warning";
    case "learning":
      return "border-l-status-stale";
    case "unknown_reset":
      return "border-l-muted-foreground";
    default:
      return "border-l-status-safe";
  }
}

export function statusTintClass(state: RiskState | "error"): string {
  switch (state) {
    case "exhausted":
    case "error":
      return "bg-status-danger/10";
    case "at_risk":
      return "bg-status-warning/10";
    case "learning":
      return "bg-status-stale/10";
    case "unknown_reset":
      return "bg-secondary/60";
    default:
      return "bg-status-safe/10";
  }
}

export function statusText(state: RiskState): string {
  return i18n.t(`status.${STATUS_I18N_KEY[state] ?? "unknown"}`);
}

export function statusTextShort(state: RiskState): string {
  return i18n.t(`status.short.${STATUS_I18N_KEY[state] ?? "unknown"}`);
}

export function formatTimeMarginShort(ms: number | null): string {
  if (ms == null) return "";
  const t = i18n.t.bind(i18n);
  const diff = ms - Date.now();
  if (diff <= 0) return t("format.zeroMinutes");
  const hours = Math.floor(diff / 3600000);
  const mins = Math.floor((diff % 3600000) / 60000);
  if (hours > 0) {
    return mins > 0
      ? t("format.hoursMinutesShort", { h: hours, m: mins })
      : t("format.hoursShort", { h: hours });
  }
  return t("format.minutesShort", { n: mins });
}

function progressOverlayColor(tier: TierDashboard): string {
  if (tier.forecast.state === "exhausted" || tier.quota.utilization >= 90) {
    return "bg-status-danger";
  }
  if (tier.forecast.state === "at_risk" || tier.quota.utilization >= 70) {
    return "bg-status-warning";
  }
  return "bg-status-safe";
}

function utilizationTextClass(tier: TierDashboard): string {
  if (tier.quota.unlimited) return "text-status-safe";
  if (tier.forecast.state === "exhausted" || tier.quota.utilization >= 90) {
    return "text-status-danger";
  }
  if (tier.forecast.state === "at_risk" || tier.quota.utilization >= 70) {
    return "text-status-warning";
  }
  return "text-foreground";
}

// ── QuotaTierList component ─────────────────────────────────────────

export interface QuotaTierListProps {
  tiers: TierDashboard[];
  selectedTierId?: string | null;
  onSelectTier?: (tier: TierDashboard) => void;
  /** When false, rows render as non-interactive divs (e.g. tray panel). */
  interactive?: boolean;
  /** Compact mode tightens paddings/text for the tray surface. */
  compact?: boolean;
  className?: string;
}

export function QuotaTierList({
  tiers,
  selectedTierId,
  onSelectTier,
  interactive = true,
  compact = false,
  className,
}: QuotaTierListProps) {
  const { t } = useTranslation();
  const sorted = React.useMemo(() => sortTiers(tiers), [tiers]);

  if (sorted.length === 0) {
    return null;
  }

  return (
    <div
      className={cn(compact ? "flex flex-col gap-0.5" : "flex flex-col gap-1.5", className)}
      data-testid="quota-tier-list"
      data-tier-count={sorted.length}
    >
            {sorted.map((tier) => {
        const quota = tier.quota;
        const isUnlimited = quota.unlimited;
        const isSelected = selectedTierId === quota.id;
        const absolute = formatAbsoluteQuota(quota);
        const rateText =
          tier.forecast.state === "learning"
            ? t("format.rateAnalyzing")
            : t("format.ratePerHour", { rate: tier.forecast.ratePerHour.toFixed(1) });
        const resetText = quota.resetsAt
          ? t("format.resetsIn", {
              time: compact
                ? formatTimeMarginShort(quota.resetsAt)
                : formatTimeMargin(quota.resetsAt),
            })
          : t("format.resetUnknown");
        const exhaustionText =
          tier.forecast.exhaustionAt != null
            ? t("format.exhaustsIn", {
                time: compact
                  ? formatTimeMarginShort(tier.forecast.exhaustionAt)
                  : formatTimeMargin(tier.forecast.exhaustionAt),
              })
            : null;

        const primaryMeta = isUnlimited
          ? t("status.safe")
          : tier.forecast.state === "exhausted"
            ? t("status.exhausted")
            : tier.forecast.state === "at_risk" && exhaustionText
              ? exhaustionText
              : resetText;
        const primaryMetaClass =
          tier.forecast.state === "exhausted" || tier.forecast.state === "error"
            ? "text-status-danger font-medium"
            : tier.forecast.state === "at_risk"
              ? "text-status-warning font-medium"
              : "text-muted-foreground";

        const body = compact ? (
          <>
            <div className="flex items-center justify-between gap-2 text-[10px] leading-tight">
              <span className="font-medium text-foreground truncate min-w-0">
                {quota.label}
              </span>
              {isUnlimited ? (
                <span className="font-bold tabular-nums text-status-safe shrink-0">
                  {t("overview.unlimitedQuota")}
                </span>
              ) : (
                <span
                  className={cn(
                    "font-bold tabular-nums shrink-0",
                    utilizationTextClass(tier)
                  )}
                >
                  {quota.utilization.toFixed(0)}%
                </span>
              )}
            </div>

            {!isUnlimited && (
              <div className="relative h-1 w-full rounded-full bg-secondary/80 overflow-hidden">
                <div
                  className={cn("absolute inset-y-0 left-0 rounded-full", progressOverlayColor(tier))}
                  style={{ width: `${Math.min(100, Math.max(0, quota.utilization))}%` }}
                />
              </div>
            )}

            <div className="flex items-center justify-between gap-2 text-[9px] leading-tight">
              <span className={cn("truncate", primaryMetaClass)}>{primaryMeta}</span>
              {!isUnlimited &&
              tier.forecast.state !== "at_risk" &&
              tier.forecast.state !== "exhausted" ? (
                <span className="text-muted-foreground/80 tabular-nums shrink-0">
                  {rateText}
                </span>
              ) : absolute ? (
                <span className="text-muted-foreground/80 tabular-nums shrink-0">
                  {absolute}
                </span>
              ) : null}
            </div>
          </>
        ) : (
          <>
            <div className="flex items-center justify-between gap-2 text-[11px]">
              <span className="font-medium text-foreground truncate">
                {quota.label}
              </span>
              {isUnlimited ? (
                <span className="font-semibold text-status-safe">{t("overview.unlimitedQuota")}</span>
              ) : (
                <span className="font-semibold text-foreground tabular-nums">
                  {quota.utilization.toFixed(1)}%
                </span>
              )}
            </div>

            {!isUnlimited && (
              <div className="relative">
                <div className="w-full rounded-full bg-secondary h-1.5" />
                <div
                  className={cn(
                    "absolute top-0 left-0 rounded-full h-1.5",
                    progressOverlayColor(tier)
                  )}
                  style={{ width: `${Math.min(100, Math.max(0, quota.utilization))}%` }}
                />
              </div>
            )}

            {absolute && (
              <p className="text-muted-foreground tabular-nums text-[10px]">
                {absolute}
              </p>
            )}

            <div className="flex justify-between text-muted-foreground text-[10px]">
              <span>{isUnlimited ? t("status.safe") : rateText}</span>
              <span className={cn(tier.forecast.state === "at_risk" && "text-status-warning font-medium")}>
                {tier.forecast.state === "at_risk" && exhaustionText
                  ? exhaustionText
                  : resetText}
              </span>
            </div>
          </>
        );

        const baseClass = cn(
          "w-full text-left flex flex-col transition-colors",
          compact
            ? cn(
                "gap-0.5 rounded-sm border-0 bg-transparent px-1.5 py-1",
                (tier.forecast.state === "exhausted" ||
                  tier.forecast.state === "at_risk" ||
                  tier.forecast.state === "error") &&
                  statusTintClass(tier.forecast.state),
                isSelected && "bg-secondary/50"
              )
            : cn(
                "gap-1 rounded-md border p-2.5",
                isSelected
                  ? "border-foreground/40 bg-secondary/60"
                  : "border-border/60 bg-card/30 hover:bg-card/55"
              ),
          !interactive && "cursor-default"
        );

        if (!interactive || !onSelectTier) {
          return (
            <div
              key={quota.id}
              className={baseClass}
              data-tier-id={quota.id}
              data-tier-state={tier.forecast.state}
              data-tier-unlimited={isUnlimited ? "true" : "false"}
            >
              {body}
            </div>
          );
        }

        return (
          <button
            key={quota.id}
            type="button"
            onClick={() => onSelectTier(tier)}
            className={baseClass}
            data-tier-id={quota.id}
            data-tier-state={tier.forecast.state}
            data-tier-unlimited={isUnlimited ? "true" : "false"}
          >
            {body}
          </button>
        );
      })}

    </div>
  );
}
