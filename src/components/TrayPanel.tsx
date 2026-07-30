import * as React from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, AlertTriangle, ExternalLink } from "lucide-react";

import { openMainWindow } from "@/lib/backend";
import { useDashboardSnapshot } from "@/hooks/useDashboardSnapshot";
import {
  QuotaTierList,
  riskSeverity,
  statusColorClass,
  statusText,
  worstTier,
} from "@/components/QuotaTierList";
import { cn } from "@/lib/utils";
import type { AccountDashboard, TierDashboard } from "@/types";

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

  return (
    <div
      data-account-id={account.account.id}
      className="rounded-md border border-border/60 bg-card/30 p-2 space-y-1.5"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5 min-w-0">
          <span
            className={cn(
              "h-1.5 w-1.5 rounded-full shrink-0",
              statusDot(dotState)
            )}
          />
          <span className="text-[11px] font-semibold text-foreground truncate">
            {account.account.displayName}
          </span>
        </div>
        {hasError ? (
          <span className="text-[9px] font-medium text-status-danger uppercase">
            {account.credentialStatus === "expired"
              ? t("status.credentialExpired")
              : account.credentialStatus === "unavailable"
              ? t("status.credentialUnavailable")
              : t("status.credentialMisconfigured")}
          </span>
        ) : w ? (
          <span
            className={cn(
              "text-[9px] font-medium px-1.5 py-0 rounded border",
              statusColorClass(w.forecast.state)
            )}
          >
            {statusText(w.forecast.state)}
          </span>
        ) : null}
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

  if (isLoading) {
    return (
      <div className="h-screen w-full bg-background text-foreground flex flex-col text-xs">
        <div className="p-3 border-b border-border/60 text-muted-foreground">
          {t("tray.loading")}
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen w-full overflow-hidden bg-background text-foreground flex flex-col text-xs">
      {/* Header */}
      <div className="px-3 py-2 border-b border-border/60 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <img src="/last-token.svg" alt={t("app.logoAlt")} className="h-4 w-4 rounded object-contain" />
          <span className="text-[12px] font-bold tracking-tight">{t("app.name")}</span>
        </div>
        <button
          type="button"
          onClick={handleRefresh}
          disabled={isRefreshing}
          aria-label={t("tray.refreshAria")}
          className="h-6 w-6 inline-flex items-center justify-center rounded-md border border-border/60 hover:bg-card disabled:opacity-50"
        >
          <RefreshCw className={cn("h-3 w-3", isRefreshing && "animate-spin")} />
        </button>
      </div>

      {/* Global risk line */}
      <div className="px-3 py-1.5 border-b border-border/60 flex items-center gap-2 text-[10px]">
        {worst ? (
          <>
            <span className={cn("h-1.5 w-1.5 rounded-full", statusDot(worst.forecast.state))} />
            <span className="text-muted-foreground">{t("tray.earliestRisk")}</span>
            <span
              className={cn(
                "font-semibold px-1.5 py-0 rounded border",
                statusColorClass(worst.forecast.state)
              )}
            >
              {statusText(worst.forecast.state)}
            </span>
            <span className="text-foreground/80 truncate">
              {worst.quota.label}
            </span>
          </>
        ) : dashboard && dashboard.accounts.length > 0 ? (
          <>
            <span className="h-1.5 w-1.5 rounded-full bg-status-safe" />
            <span className="text-muted-foreground">{t("tray.allSafe")}</span>
          </>
        ) : (
          <>
            <AlertTriangle className="h-3 w-3 text-muted-foreground" />
            <span className="text-muted-foreground">{t("tray.noProviders")}</span>
          </>
        )}
      </div>

      {/* Body — scrollable account list */}
      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {orderedAccounts.length > 0 ? (
          orderedAccounts.map((acc) => <TrayAccountRow key={acc.account.id} account={acc} />)
        ) : (
          <p className="text-[10px] text-muted-foreground px-1 py-3 text-center">
            {t("tray.emptyHint")}
          </p>
        )}
      </div>

      {/* Footer */}
      <div className="px-2 py-2 border-t border-border/60 flex items-center gap-2">
        <button
          type="button"
          onClick={handleRefresh}
          disabled={isRefreshing}
          className="flex-1 h-7 inline-flex items-center justify-center rounded-md border border-border/60 bg-card/40 hover:bg-card/70 text-[11px] font-medium disabled:opacity-50"
        >
          <RefreshCw className={cn("h-3 w-3 mr-1", isRefreshing && "animate-spin")} />
          {t("tray.refresh")}
        </button>
        <button
          type="button"
          onClick={() => {
            void openMainWindow();
          }}
          className="flex-1 h-7 inline-flex items-center justify-center rounded-md bg-foreground text-background hover:opacity-90 text-[11px] font-medium"
        >
          <ExternalLink className="h-3 w-3 mr-1" />
          {t("tray.openMain")}
        </button>
      </div>
    </div>
  );
}
