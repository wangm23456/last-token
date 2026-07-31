import * as React from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, AlertTriangle, ExternalLink, LogOut } from "lucide-react";

import { openMainWindow, quitApp, setTrayPanelHeight } from "@/lib/backend";
import { useDashboardSnapshot } from "@/hooks/useDashboardSnapshot";
import {
  QuotaTierList,
  formatTimeMarginShort,
  riskSeverity,
  statusAccentClass,
  statusColorClass,
  statusTextShort,
  statusTintClass,
  worstTier,
} from "@/components/QuotaTierList";
import { cn } from "@/lib/utils";
import type { AccountDashboard, RiskState, TierDashboard } from "@/types";


const TRAY_PANEL_MIN_HEIGHT = 96;
const TRAY_PANEL_MAX_HEIGHT = 520;

function worstAcrossAccounts(accounts: AccountDashboard[]): TierDashboard | null {
  let worst: TierDashboard | null = null;
  let worstSev = -1;
  for (const acc of accounts) {
    if (acc.credentialStatus !== "valid" || acc.error) continue;
    const w = worstTier(acc.tiers);
    if (!w) continue;
    const sev = riskSeverity(w.forecast.state);
    // Higher severity wins; on ties, earlier exhaustion/reset wins, then
    // stable tier identity (account id + tier id) for a deterministic result.
    if (
      worst === null ||
      sev > worstSev ||
      (sev === worstSev &&
        (w.forecast.exhaustionAt ?? w.quota.resetsAt ?? Number.POSITIVE_INFINITY) <
          (worst.forecast.exhaustionAt ?? worst.quota.resetsAt ?? Number.POSITIVE_INFINITY))
    ) {
      worst = w;
      worstSev = sev;
    }
  }
  return worst;
}

function statusDot(state: TierDashboard["forecast"]["state"] | "error"): string {
  switch (state) {
    case "exhausted":
      return "bg-status-danger";
    case "at_risk":
      return "bg-status-warning";
    case "unknown_reset":
      return "bg-muted-foreground";
    case "learning":
      return "bg-status-stale";
    case "error":
      return "bg-status-danger";
    default:
      return "bg-status-safe";
  }
}

function riskMeta(tier: TierDashboard, t: (key: string, opts?: Record<string, unknown>) => string): string | null {
  if (tier.quota.unlimited) return null;
  if (tier.forecast.state === "at_risk" && tier.forecast.exhaustionAt != null) {
    return t("format.exhaustsIn", {
      time: formatTimeMarginShort(tier.forecast.exhaustionAt),
    });
  }
  if (tier.quota.resetsAt != null) {
    return t("format.resetsIn", {
      time: formatTimeMarginShort(tier.quota.resetsAt),
    });
  }
  if (tier.forecast.state === "exhausted") return t("status.short.exhausted");
  return null;
}

interface AccountRowProps {
  account: AccountDashboard;
}

function TrayAccountRow({ account }: AccountRowProps) {
  const { t } = useTranslation();
  const hasError = account.credentialStatus !== "valid" || !!account.error;
  const w = worstTier(account.tiers);
  const dotState: "error" | TierDashboard["forecast"]["state"] = hasError
    ? "error"
    : w
      ? w.forecast.state
      : "safe";
  const accentState: RiskState | "error" = hasError ? "error" : (w?.forecast.state ?? "safe");

  return (
    <div
      data-account-id={account.account.id}
      className={cn(
        "rounded-md border border-border/50 bg-card/25 border-l-2 pl-2 pr-1.5 py-1.5 space-y-1",
        statusAccentClass(accentState)
      )}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5 min-w-0">
          <span className={cn("h-1.5 w-1.5 rounded-full shrink-0", statusDot(dotState))} />
          <span className="text-[11px] font-semibold text-foreground truncate">
            {account.account.displayName}
          </span>
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          {!hasError && w && !w.quota.unlimited ? (
            <span
              className={cn(
                "text-[11px] font-bold tabular-nums",
                w.forecast.state === "exhausted" || w.quota.utilization >= 90
                  ? "text-status-danger"
                  : w.forecast.state === "at_risk" || w.quota.utilization >= 70
                    ? "text-status-warning"
                    : "text-foreground"
              )}
            >
              {w.quota.utilization.toFixed(0)}%
            </span>
          ) : null}
          {hasError ? (
            <span className="text-[9px] font-semibold text-status-danger">
              {account.credentialStatus === "expired"
                ? t("status.credentialExpired")
                : account.credentialStatus === "unavailable"
                  ? t("status.credentialUnavailable")
                  : t("status.credentialMisconfigured")}
            </span>
          ) : w ? (
            <span
              className={cn(
                "text-[9px] font-semibold px-1 py-0 rounded border",
                statusColorClass(w.forecast.state)
              )}
            >
              {statusTextShort(w.forecast.state)}
            </span>
          ) : null}
        </div>
      </div>

      {hasError ? (
        <p className="text-[10px] text-status-danger/90 font-medium leading-snug">
          {account.error || t("tray.credentialError")}
        </p>
      ) : account.tiers.length > 0 ? (
        <QuotaTierList tiers={account.tiers} compact interactive={false} />
      ) : (
        <p className="text-[10px] text-muted-foreground">{t("tray.noQuota")}</p>
      )}
    </div>
  );
}

export function TrayPanel() {
  const { t } = useTranslation();
  const { query, refreshMutation } = useDashboardSnapshot();
  const { data: dashboard, isLoading } = query;
  const isRefreshing = refreshMutation.isPending || dashboard?.refreshInProgress === true;
  const rootRef = React.useRef<HTMLDivElement>(null);
  const lastHeightRef = React.useRef(0);
  const [heightCapped, setHeightCapped] = React.useState(false);

  // Tray runs in a separate webview, so its settings cache can stay stale after
  // overview drag. Dashboard snapshots are already ordered by the backend.
  const orderedAccounts = dashboard?.accounts ?? [];

  const worst = React.useMemo(() => {
    if (!dashboard) return null;
    return worstAcrossAccounts(dashboard.accounts);
  }, [dashboard]);

  const handleRefresh = React.useCallback(() => {
    if (isRefreshing) return;
    refreshMutation.mutate();
  }, [isRefreshing, refreshMutation]);

  // Width stays fixed; height follows content, clamped to [min, max].
  // When capped, the account list scrolls inside the max window height.
  React.useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root || typeof ResizeObserver === "undefined") {
      return;
    }

    let frame = 0;
    const syncHeight = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const natural = Math.ceil(root.scrollHeight);
        const capped = natural > TRAY_PANEL_MAX_HEIGHT;
        setHeightCapped(capped);
        const target = Math.min(
          Math.max(natural, TRAY_PANEL_MIN_HEIGHT),
          TRAY_PANEL_MAX_HEIGHT,
        );
        if (Math.abs(target - lastHeightRef.current) < 1) {
          return;
        }
        lastHeightRef.current = target;
        void setTrayPanelHeight(target).catch(() => {
          // Browser preview / tests: ignore missing Tauri bridge.
        });
      });
    };

    const observer = new ResizeObserver(() => {
      syncHeight();
    });
    observer.observe(root);
    syncHeight();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [dashboard, isLoading, orderedAccounts.length]);

  const worstMeta = worst ? riskMeta(worst, t) : null;

  return (
    <div
      ref={rootRef}
      className={cn(
        "w-full bg-background text-foreground flex flex-col text-xs",
        heightCapped ? "h-screen overflow-hidden" : "h-auto overflow-visible"
      )}
    >
      {isLoading ? (
        <div className="px-2.5 py-2 border-b border-border/60 text-muted-foreground">
          {t("tray.loading")}
        </div>
      ) : (
        <>
          {/* Header */}
          <div className="px-2.5 py-1.5 border-b border-border/60 flex items-center justify-between gap-2">
            <div className="flex items-center gap-1.5 min-w-0">
              <img
                src="/last-token.svg"
                alt={t("app.logoAlt")}
                className="h-3.5 w-3.5 rounded object-contain shrink-0"
              />
              <span className="text-[11px] font-bold tracking-tight truncate">{t("app.name")}</span>
            </div>
            <button
              type="button"
              onClick={handleRefresh}
              disabled={isRefreshing}
              aria-label={t("tray.refreshAria")}
              className="h-5 w-5 inline-flex items-center justify-center rounded border border-border/60 hover:bg-card disabled:opacity-50 shrink-0"
            >
              <RefreshCw className={cn("h-3 w-3", isRefreshing && "animate-spin")} />
            </button>
          </div>

          {/* Global risk line — tinted for glanceability */}
          <div
            className={cn(
              "px-2.5 py-1.5 border-b border-border/60 flex items-center gap-1.5 text-[10px] min-h-[28px]",
              worst ? statusTintClass(worst.forecast.state) : "bg-status-safe/5"
            )}
          >
            {worst ? (
              <>
                <span className={cn("h-1.5 w-1.5 rounded-full shrink-0", statusDot(worst.forecast.state))} />
                <span
                  className={cn(
                    "font-semibold px-1 py-0 rounded border shrink-0",
                    statusColorClass(worst.forecast.state)
                  )}
                >
                  {statusTextShort(worst.forecast.state)}
                </span>
                {!worst.quota.unlimited ? (
                  <span
                    className={cn(
                      "font-bold tabular-nums shrink-0",
                      worst.forecast.state === "exhausted" || worst.quota.utilization >= 90
                        ? "text-status-danger"
                        : worst.forecast.state === "at_risk" || worst.quota.utilization >= 70
                          ? "text-status-warning"
                          : "text-foreground"
                    )}
                  >
                    {worst.quota.utilization.toFixed(0)}%
                  </span>
                ) : null}
                <span className="text-foreground/85 truncate min-w-0">{worst.quota.label}</span>
                {worstMeta ? (
                  <span
                    className={cn(
                      "ml-auto tabular-nums shrink-0",
                      worst.forecast.state === "exhausted" || worst.forecast.state === "at_risk"
                        ? "text-status-danger font-semibold"
                        : "text-muted-foreground"
                    )}
                  >
                    {worstMeta}
                  </span>
                ) : (
                  <span className="ml-auto text-muted-foreground shrink-0">{t("tray.earliestRisk")}</span>
                )}
              </>
            ) : dashboard && dashboard.accounts.length > 0 ? (
              <>
                <span className="h-1.5 w-1.5 rounded-full bg-status-safe shrink-0" />
                <span className="font-medium text-status-safe">{t("tray.allSafe")}</span>
              </>
            ) : (
              <>
                <AlertTriangle className="h-3 w-3 text-muted-foreground shrink-0" />
                <span className="text-muted-foreground">{t("tray.noProviders")}</span>
              </>
            )}
          </div>

          {/* Body — denser account list */}
          <div
            className={cn(
              "px-1.5 py-1.5 space-y-1.5",
              heightCapped ? "flex-1 min-h-0 overflow-y-auto" : ""
            )}
          >
            {orderedAccounts.length > 0 ? (
              orderedAccounts.map((acc) => <TrayAccountRow key={acc.account.id} account={acc} />)
            ) : (
              <p className="text-[10px] text-muted-foreground px-1 py-3 text-center">
                {t("tray.emptyHint")}
              </p>
            )}
          </div>

          {/* Footer — single compact row */}
          <div className="px-1.5 py-1.5 border-t border-border/60 flex items-center gap-1.5">
            <button
              type="button"
              onClick={handleRefresh}
              disabled={isRefreshing}
              className="flex-1 h-6 inline-flex items-center justify-center rounded-md border border-border/60 bg-card/40 hover:bg-card/70 text-[10px] font-medium disabled:opacity-50"
            >
              <RefreshCw className={cn("h-3 w-3 mr-1", isRefreshing && "animate-spin")} />
              {t("tray.refresh")}
            </button>
            <button
              type="button"
              onClick={() => {
                void openMainWindow();
              }}
              aria-label={t("tray.openMainAria")}
              className="flex-1 h-6 inline-flex items-center justify-center rounded-md bg-foreground text-background hover:opacity-90 text-[10px] font-medium"
            >
              <ExternalLink className="h-3 w-3 mr-1" />
              {t("tray.openMain")}
            </button>
            <button
              type="button"
              onClick={() => {
                void quitApp();
              }}
              aria-label={t("tray.quitAria")}
              className="h-6 px-2 inline-flex items-center justify-center rounded-md border border-border/60 text-muted-foreground hover:text-status-danger hover:border-status-danger/40 hover:bg-status-danger/5 text-[10px] font-medium"
            >
              <LogOut className="h-3 w-3 mr-1" />
              {t("tray.quit")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
