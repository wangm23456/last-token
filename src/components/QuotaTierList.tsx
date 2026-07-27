import * as React from "react";
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
  const diff = ms - Date.now();
  if (diff <= 0) return "0分钟";
  const hours = Math.floor(diff / 3600000);
  const mins = Math.floor((diff % 3600000) / 60000);
  if (hours > 0) {
    return `${hours}小时${mins}分钟`;
  }
  return `${mins}分钟`;
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

export function statusText(state: RiskState): string {
  switch (state) {
    case "exhausted":
      return "已耗尽";
    case "at_risk":
      return "有耗尽风险";
    case "unknown_reset":
      return "重置时间未知";
    case "learning":
      return "正在分析速率";
    case "safe":
      return "额度充足";
    case "error":
      return "查询错误";
    default:
      return "未知";
  }
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
  const sorted = React.useMemo(() => sortTiers(tiers), [tiers]);

  if (sorted.length === 0) {
    return null;
  }

  return (
    <div
      className={cn("flex flex-col gap-1.5", className)}
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
            ? "分析中..."
            : `消耗速率: ${tier.forecast.ratePerHour.toFixed(1)}%/时`;
        const resetText = quota.resetsAt
          ? `${formatTimeMargin(quota.resetsAt)} 后重置`
          : "周期重置未指定";

        const body = (
          <>
            <div
              className={cn(
                "flex items-center justify-between gap-2",
                compact ? "text-[10px]" : "text-[11px]"
              )}
            >
              <span className="font-medium text-foreground truncate">
                {quota.label}
              </span>
              {isUnlimited ? (
                <span className="font-semibold text-status-safe">无限额度</span>
              ) : (
                <span className="font-semibold text-foreground tabular-nums">
                  {quota.utilization.toFixed(1)}%
                </span>
              )}
            </div>

            {!isUnlimited && (
              <div className="relative">
                <div
                  className={cn(
                    "w-full rounded-full bg-secondary",
                    compact ? "h-1" : "h-1.5"
                  )}
                />
                <div
                  className={cn(
                    "absolute top-0 left-0 rounded-full",
                    compact ? "h-1" : "h-1.5",
                    progressOverlayColor(tier)
                  )}
                  style={{ width: `${Math.min(100, Math.max(0, quota.utilization))}%` }}
                />
              </div>
            )}

            {absolute && (
              <p
                className={cn(
                  "text-muted-foreground tabular-nums",
                  compact ? "text-[9px]" : "text-[10px]"
                )}
              >
                {absolute}
              </p>
            )}

            <div
              className={cn(
                "flex justify-between text-muted-foreground",
                compact ? "text-[9px]" : "text-[10px]"
              )}
            >
              <span>{isUnlimited ? "无耗尽风险" : rateText}</span>
              <span>{resetText}</span>
            </div>
          </>
        );

        const baseClass = cn(
          "w-full text-left flex flex-col gap-1 rounded-md border transition-colors",
          compact ? "p-2" : "p-2.5",
          isSelected
            ? "border-foreground/40 bg-secondary/60"
            : "border-border/60 bg-card/30 hover:bg-card/55",
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
