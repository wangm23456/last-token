import * as React from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Plus,
  Trash2,
  Edit2,
  RefreshCw,
  CheckCircle,
  AlertCircle,
  Eye,
  EyeOff,
  Copy,
  ExternalLink,
  Wand2,
} from "lucide-react";

import {
  listAccounts,
  saveAccount,
  deleteAccount,
  discoverEnvAccounts,
  probeCliCredentials,
  startCopilotDeviceFlow,
  pollCopilotDeviceFlow,
  getSettings,
  getDashboard,
  requestNotificationPermission,
} from "@/lib/backend";
import { applyAccountOrder, applyIdOrder } from "@/lib/accountOrder";
import { getErrorMessage } from "@/lib/errors";
import { tierSortKey } from "@/components/QuotaTierList";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from "@/components/ui/alert-dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Field } from "@/components/ui/field";
import { Empty } from "@/components/ui/empty";
import { AlertRule, PublicAccount, ProviderConfig, ProviderKind, SecretPayload } from "@/types";

export function ProvidersTab() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [isFormOpen, setIsFormOpen] = React.useState(false);
  const [editingAccount, setEditingAccount] = React.useState<PublicAccount | null>(null);
  const [deletingAccount, setDeletingAccount] = React.useState<PublicAccount | null>(null);

  // Form Fields State
  const [displayName, setDisplayName] = React.useState("");
  const [enabled, setEnabled] = React.useState(true);
  const [providerType, setProviderType] = React.useState<ProviderKind>("kimi");

  // Provider Specific Configs
  const [githubDomain, setGithubDomain] = React.useState("");
  const [zhipuRegion, setZhipuRegion] = React.useState("cn");
  const [zhipuTeamOrgId, setZhipuTeamOrgId] = React.useState("");
  const [zhipuTeamProjId, setZhipuTeamProjId] = React.useState("");
  const [minimaxRegion, setMinimaxRegion] = React.useState("cn");
  const [zenmuxUrl, setZenmuxUrl] = React.useState("");
  const [volcengineRegion, setVolcengineRegion] = React.useState("cn-beijing");

  // Secrets State
  const [apiKey, setApiKey] = React.useState("");
  const [accessKeyId, setAccessKeyId] = React.useState("");
  const [secretAccessKey, setSecretAccessKey] = React.useState("");
  const [showSecret, setShowSecret] = React.useState(false);

  // Copilot Device Code State
  const [copilotStatus, setCopilotStatus] = React.useState<"idle" | "requesting" | "polling" | "success" | "error" | "expired">("idle");
  const [copilotDeviceCode, setCopilotDeviceCode] = React.useState("");
  const [copilotUserCode, setCopilotUserCode] = React.useState("");
  const [copilotVerifyUri, setCopilotVerifyUri] = React.useState("");
  const [copilotError, setCopilotError] = React.useState("");
  const copilotInterval = React.useRef<number | null>(null);

  const [isImporting, setIsImporting] = React.useState(false);
  // Per-tier alert rule drafts (only relevant when editing existing accounts)
  const [alertDrafts, setAlertDrafts] = React.useState<AlertRule[]>([]);
  type AlertMeta = { label: string; unlimited: boolean; missing: boolean };
  const [alertMeta, setAlertMeta] = React.useState<Record<string, AlertMeta>>({});
  const [alertsOnlyMode, setAlertsOnlyMode] = React.useState(false);

  // 1. Fetch Accounts
  const { data: accounts = [], isLoading } = useQuery({
    queryKey: ["accounts"],
    queryFn: listAccounts,
  });

  // 2. Fetch CLI probes
  const { data: cliProbes = [], refetch: refetchProbes, isFetching: isProbing } = useQuery({
    queryKey: ["cliProbes"],
    queryFn: probeCliCredentials,
    refetchOnMount: true,
  });

  // 3. Save Account Mutation
  const saveMutation = useMutation({
    mutationFn: saveAccount,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setIsFormOpen(false);
      resetForm();
      toast.success(t("providers.toasts.saved"));
    },
    onError: (err) => {
      toast.error(t("providers.toasts.saveFailed", { message: getErrorMessage(err) }));
    },
  });

  // 4. Delete Account Mutation
  const deleteMutation = useMutation({
    mutationFn: deleteAccount,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setDeletingAccount(null);
      toast.success(t("providers.toasts.deleted"));
    },
    onError: (err) => {
      toast.error(t("providers.toasts.deleteFailed", { message: getErrorMessage(err) }));
    },
  });

  // 5. Discover Env Accounts Mutation
  const discoverMutation = useMutation({
    mutationFn: discoverEnvAccounts,
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      if (created.length === 0) {
        toast.info(t("providers.toasts.noNewEnv"));
      } else {
        toast.success(
          t("providers.toasts.autoAdded", {
            count: created.length,
            names: created.map((a) => a.displayName).join(", "),
          }),
        );
      }
    },
    onError: (err) => {
      toast.error(t("providers.toasts.discoverFailed", { message: getErrorMessage(err) }));
    },
  });

  const handleOneClickImport = async () => {
    setIsImporting(true);
    try {
      const [probes, envAccounts] = await Promise.all([
        probeCliCredentials(),
        discoverEnvAccounts(),
      ]);

      const existingCli = accounts
        .filter((a) => a.credentialSource === "cli_auto")
        .map((a) => a.id);
      const newCli = probes.filter(
        (p) => p.status === "valid" && !existingCli.includes(p.accountId ?? "")
      );

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["accounts"] }),
        queryClient.invalidateQueries({ queryKey: ["dashboard"] }),
        queryClient.invalidateQueries({ queryKey: ["cliProbes"] }),
      ]);

      const cliCount = newCli.length;
      const envCount = envAccounts.length;

      if (cliCount > 0 || envCount > 0) {
        const parts: string[] = [];
        if (cliCount > 0) {
          parts.push(
            t("providers.toasts.importCliPart", {
              names: newCli.map((c) => t(`providers.cli.${c.provider}Name`)).join(", "),
            }),
          );
        }
        if (envCount > 0) {
          parts.push(
            t("providers.toasts.importEnvPart", {
              names: envAccounts.map((e) => e.displayName).join(", "),
            }),
          );
        }
        toast.success(
          t("providers.toasts.importOk", { parts: parts.join(t("providers.toasts.importJoin")) }),
        );
      } else {
        toast.info(t("providers.toasts.importEmpty"));
      }
    } catch (err) {
      toast.error(t("providers.toasts.importFailed", { message: getErrorMessage(err) }));
    } finally {
      setIsImporting(false);
    }
  };


  const buildAlertDrafts = (acc: PublicAccount) => {
    const dashAcc = dashboard?.accounts.find((a) => a.account.id === acc.id);
    const tiers = dashAcc?.tiers ?? [];
    const byId = new Map(acc.alertRules.map((r) => [r.tierId, r]));
    const meta: Record<string, { label: string; unlimited: boolean; missing: boolean }> = {};
    const drafts: AlertRule[] = [];

    for (const t of tiers) {
      const existing = byId.get(t.quota.id);
      meta[t.quota.id] = {
        label: t.quota.label,
        unlimited: t.quota.unlimited,
        missing: false,
      };
      if (t.quota.unlimited) {
        drafts.push({
          tierId: t.quota.id,
          enabled: false,
          thresholdPercent: existing?.thresholdPercent ?? 80,
        });
      } else {
        drafts.push(
          existing ?? {
            tierId: t.quota.id,
            enabled: false,
            thresholdPercent: 80,
          }
        );
      }
      byId.delete(t.quota.id);
    }

    for (const [tierId, rule] of byId) {
      meta[tierId] = { label: tierId, unlimited: false, missing: true };
      drafts.push(rule);
    }

    drafts.sort((a, b) => {
      const [ra, sa] = tierSortKey(a.tierId);
      const [rb, sb] = tierSortKey(b.tierId);
      return ra - rb || sa.localeCompare(sb) || a.tierId.localeCompare(b.tierId);
    });

    setAlertMeta(meta);
    setAlertDrafts(drafts);
  };

  const handleEdit = (acc: PublicAccount, opts?: { alertsOnly?: boolean }) => {
    setEditingAccount(acc);
    setAlertsOnlyMode(!!opts?.alertsOnly || acc.credentialSource === "cli_auto");
    setDisplayName(acc.displayName);
    setEnabled(acc.enabled);
    setProviderType(acc.provider);

    // Unpack config
    switch (acc.config.type) {
      case "copilot":
        setGithubDomain(acc.config.githubDomain || "");
        break;
      case "zhipu":
        setZhipuRegion(acc.config.region);
        break;
      case "zhipu_team":
        setZhipuTeamOrgId(acc.config.organizationId);
        setZhipuTeamProjId(acc.config.projectId);
        break;
      case "minimax":
        setMinimaxRegion(acc.config.region);
        break;
      case "zenmux":
        setZenmuxUrl(acc.config.quotaUrl);
        break;
      case "volcengine":
        setVolcengineRegion(acc.config.region);
        break;
    }

    // Secrets remain empty (meaning preserve)
    setApiKey("");
    setAccessKeyId("");
    setSecretAccessKey("");

    buildAlertDrafts(acc);
    setIsFormOpen(true);
  };

  const handleAddNew = () => {
    setEditingAccount(null);
    setAlertsOnlyMode(false);
    setAlertDrafts([]);
    setAlertMeta({});
    resetForm();
    setIsFormOpen(true);
  };

  const resetForm = () => {
    setAlertsOnlyMode(false);
    setAlertDrafts([]);
    setAlertMeta({});
    setDisplayName("");
    setEnabled(true);
    setProviderType("kimi");
    setGithubDomain("");
    setZhipuRegion("cn");
    setZhipuTeamOrgId("");
    setZhipuTeamProjId("");
    setMinimaxRegion("cn");
    setZenmuxUrl("");
    setVolcengineRegion("cn-beijing");
    setApiKey("");
    setAccessKeyId("");
    setSecretAccessKey("");
    setShowSecret(false);

    // Clear copilot polling
    setCopilotStatus("idle");
    setCopilotDeviceCode("");
    setCopilotUserCode("");
    setCopilotVerifyUri("");
    setCopilotError("");
    if (copilotInterval.current) {
      window.clearInterval(copilotInterval.current);
      copilotInterval.current = null;
    }
  };

  const alertRulesValid = alertDrafts.every((r) => {
    const meta = alertMeta[r.tierId];
    if (meta?.unlimited) return true;
    return Number.isInteger(r.thresholdPercent) && r.thresholdPercent >= 1 && r.thresholdPercent <= 99;
  });

  const handleAlertToggle = async (tierId: string, enabled: boolean) => {
    const meta = alertMeta[tierId];
    if (meta?.unlimited) return;
    if (enabled) {
      try {
        const granted = await requestNotificationPermission();
        if (!granted) {
          toast.error(t("providers.toasts.notificationDenied"));
          return;
        }
      } catch (err) {
        toast.error(t("providers.toasts.notificationDenied"));
        return;
      }
    }
    setAlertDrafts((prev) =>
      prev.map((r) => (r.tierId === tierId ? { ...r, enabled } : r))
    );
  };

  const handleFormSubmit = () => {
    if (editingAccount && !alertRulesValid) {
      toast.error(t("providers.toasts.thresholdInvalid"));
      return;
    }

    if (alertsOnlyMode && editingAccount) {
      saveMutation.mutate({
        id: editingAccount.id,
        displayName: editingAccount.displayName,
        enabled: editingAccount.enabled,
        config: editingAccount.config,
        secret: undefined,
        alertRules: alertDrafts.map((r) => ({
          tierId: r.tierId,
          enabled: alertMeta[r.tierId]?.unlimited ? false : r.enabled,
          thresholdPercent: r.thresholdPercent,
        })),
      });
      return;
    }

    if (!displayName.trim()) {
      toast.error(t("providers.toasts.nameRequired"));
      return;
    }

    // ZenMux URL Validation
    if (providerType === "zenmux") {
      if (!zenmuxUrl.startsWith("https://")) {
        toast.error(t("providers.toasts.zenmuxHttps"));
        return;
      }
      if (!zenmuxUrl.includes("zenmux.")) {
        toast.error(t("providers.toasts.zenmuxDomain"));
        return;
      }
    }

    // Secret validation (required for new accounts, optional on edit)
    const isNew = !editingAccount;
    let secret: SecretPayload | undefined;

    if (providerType === "volcengine") {
      if (accessKeyId.trim() && secretAccessKey.trim()) {
        secret = {
          type: "volcengine",
          accessKeyId,
          secretAccessKey,
        };
      }
    } else if (providerType !== "copilot") {
      if (apiKey.trim()) {
        secret = {
          type: "api_key",
          apiKey,
        };
      }
    }

    // Build Config
    let config: ProviderConfig;
    switch (providerType) {
      case "claude": config = { type: "claude" }; break;
      case "codex": config = { type: "codex" }; break;
      case "gemini": config = { type: "gemini" }; break;
      case "copilot":
        config = { type: "copilot", githubDomain: githubDomain.trim() || null };
        break;
      case "kimi": config = { type: "kimi" }; break;
      case "zhipu":
        config = { type: "zhipu", region: zhipuRegion };
        break;
      case "zhipu_team":
        if (!zhipuTeamOrgId.trim() || !zhipuTeamProjId.trim()) {
          toast.error(t("providers.toasts.zhipuTeamIdRequired"));
          return;
        }
        config = {
          type: "zhipu_team",
          organizationId: zhipuTeamOrgId.trim(),
          projectId: zhipuTeamProjId.trim(),
        };
        break;
      case "minimax":
        config = { type: "minimax", region: minimaxRegion };
        break;
      case "zenmux":
        config = { type: "zenmux", quotaUrl: zenmuxUrl.trim() };
        break;
      case "volcengine":
        config = { type: "volcengine", region: volcengineRegion.trim() || "cn-beijing" };
        break;
    }

    saveMutation.mutate({
      id: editingAccount?.id,
      displayName,
      enabled,
      config,
      secret,
      alertRules: editingAccount ? alertDrafts.map((r) => ({
        tierId: r.tierId,
        enabled: alertMeta[r.tierId]?.unlimited ? false : r.enabled,
        thresholdPercent: r.thresholdPercent,
      })) : [],
    });
  };

  const handleCopilotLink = async () => {
    setCopilotStatus("requesting");
    setCopilotError("");
    try {
      const startRes = await startCopilotDeviceFlow(githubDomain.trim() || null);
      setCopilotDeviceCode(startRes.deviceCode);
      setCopilotUserCode(startRes.userCode);
      setCopilotVerifyUri(startRes.verificationUri);
      setCopilotStatus("polling");

      // Copy code to clipboard automatically; do not lie to the user if it fails.
      try {
        await navigator.clipboard.writeText(startRes.userCode);
        toast.success(t("providers.toasts.codeCopied"));
      } catch {
        toast.info(t("providers.toasts.codeCopyFailed"));
      }

      // Start Polling — interval is resettable so we can honor GitHub's
      // `slow_down` advice (which adds 5s to the suggested interval).
      let intervalSec = startRes.interval || 5;
      const stopPolling = () => {
        if (copilotInterval.current !== null) {
          window.clearInterval(copilotInterval.current);
          copilotInterval.current = null;
        }
      };
      const startPolling = (sec: number) => {
        stopPolling();
        copilotInterval.current = window.setInterval(async () => {
          try {
            const pollRes = await pollCopilotDeviceFlow(
              startRes.deviceCode,
              githubDomain.trim() || null,
              intervalSec,
            );
            if (pollRes.type === "authorized") {
              stopPolling();
              setCopilotStatus("success");
              toast.success(t("providers.toasts.linkSuccess"));
              queryClient.invalidateQueries({ queryKey: ["accounts"] });
              queryClient.invalidateQueries({ queryKey: ["dashboard"] });
              resetForm();
              setIsFormOpen(false);
            } else if (pollRes.type === "expired") {
              stopPolling();
              setCopilotStatus("expired");
              setCopilotError(t("providers.toasts.codeExpired"));
            } else if (pollRes.type === "denied") {
              stopPolling();
              setCopilotStatus("error");
              setCopilotError(pollRes.message || t("providers.toasts.codeDenied"));
            } else if (pollRes.type === "pending") {
              if (
                pollRes.retryAfterSeconds &&
                pollRes.retryAfterSeconds !== intervalSec
              ) {
                intervalSec = pollRes.retryAfterSeconds;
                startPolling(intervalSec);
              }
            }
          } catch (err) {
            stopPolling();
            setCopilotStatus("error");
            setCopilotError(
              err instanceof Error ? err.message : t("providers.toasts.codeCopyPolling"),
            );
          }
        }, sec * 1000);
      };
      startPolling(intervalSec);
    } catch (err) {
      setCopilotStatus("error");
      setCopilotError(err instanceof Error ? err.message : t("providers.toasts.codeCopyRequest"));
    }
  };

  const handleCopyCode = () => {
    navigator.clipboard
      .writeText(copilotUserCode)
      .then(() => toast.success(t("providers.toasts.codeCopyOk")))
      .catch(() => toast.error(t("providers.toasts.codeCopyManual")));
  };

  // Close timer when component unmounts
  React.useEffect(() => {
    return () => {
      if (copilotInterval.current) {
        window.clearInterval(copilotInterval.current);
      }
    };
  }, []);

  const getCliStatusIcon = (status: string) => {
    switch (status) {
      case "valid":
        return <CheckCircle className="h-4 w-4 text-status-safe" />;
      case "expired":
        return <AlertCircle className="h-4 w-4 text-status-warning" />;
      default:
        return <AlertCircle className="h-4 w-4 text-muted-foreground" />;
    }
  };

  const getCliStatusLabel = (status: string) => {
    switch (status) {
      case "valid":
        return (
          <Badge variant="outline" className="text-[10px] text-status-safe border-status-safe/20 bg-status-safe/5">
            {t("providers.cli.detected")}
          </Badge>
        );
      case "expired":
        return (
          <Badge variant="outline" className="text-[10px] text-status-warning border-status-warning/20 bg-status-warning/5">
            {t("providers.cli.expired")}
          </Badge>
        );
      case "not_found":
        return <Badge variant="outline" className="text-[10px] text-muted-foreground">{t("providers.cli.notFound")}</Badge>;
      default:
        return <Badge variant="outline" className="text-[10px] text-muted-foreground">{t("providers.cli.error")}</Badge>;
    }
  };

  const getCliSourceGuide = (provider: string) => {
    switch (provider) {
      case "claude":
        return t("providers.cli.claudeGuide");
      case "codex":
        return t("providers.cli.codexGuide");
      case "gemini":
        return t("providers.cli.geminiGuide");
      default:
        return "";
    }
  };

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  const { data: dashboard } = useQuery({
    queryKey: ["dashboard"],
    queryFn: getDashboard,
  });

  // Filter to manual / Copilot accounts, then align with overview/tray order.
  const manualAccounts = React.useMemo(() => {
    const manual = accounts.filter((a) => a.credentialSource !== "cli_auto");
    const riskOrder = dashboard
      ? applyAccountOrder(dashboard.accounts, undefined).map((acc) => acc.account.id)
      : undefined;
    return applyIdOrder(
      manual,
      (a) => a.id,
      settings?.accountOrder,
      riskOrder,
    );
  }, [accounts, dashboard, settings?.accountOrder]);

  return (
    <div className="space-y-4">
      {/* ── One-click Detect & Import Card ────────────────────────── */}
      <Card className="border-border bg-card/25 shadow-sm overflow-hidden">
        <CardContent className="p-3 flex items-center justify-between gap-3">
          <div className="space-y-0.5">
            <h4 className="text-xs font-semibold text-foreground">{t("providers.oneClickImport.title")}</h4>
            <p className="text-[10px] text-muted-foreground">
              {t("providers.oneClickImport.desc")}
            </p>
          </div>
          <Button
            size="sm"
            className="h-8 text-xs flex items-center gap-1.5 shadow-sm"
            onClick={handleOneClickImport}
            disabled={isImporting}
          >
            <Wand2 className={`h-3.5 w-3.5 ${isImporting ? "animate-spin" : ""}`} />
            {t("providers.oneClickImport.button")}
          </Button>
        </CardContent>
      </Card>

      {/* ── CLI Auto Discovery Section ────────────────────────────── */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-bold text-muted-foreground uppercase tracking-wider">{t("providers.cli.section")}</h3>
          <Button
            variant="ghost"
            size="xs"
            className="text-[11px] text-muted-foreground hover:bg-muted h-7 flex items-center gap-1"
            onClick={() => refetchProbes()}
            disabled={isProbing}
          >
            <RefreshCw className={`h-3 w-3 ${isProbing ? "animate-spin" : ""}`} />
            {t("providers.cli.rescan")}
          </Button>
        </div>

        <div className="grid gap-2">
          {cliProbes.map((probe) => (
            <Card key={probe.provider} className="border-border bg-card/20 shadow-sm">
              <CardContent className="p-3 flex items-start justify-between gap-3">
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    {getCliStatusIcon(probe.status)}
                    <span className="text-xs font-semibold text-foreground">
                      {probe.provider === "claude" ? t("providers.cli.claudeName") : probe.provider === "codex" ? t("providers.cli.codexName") : t("providers.cli.geminiName")}
                    </span>
                    {getCliStatusLabel(probe.status)}
                  </div>
                  <p className="text-[10px] text-muted-foreground leading-normal max-w-[340px]">
                    {getCliSourceGuide(probe.provider)}
                  </p>
                  {probe.message && (
                    <p className="text-[10px] text-status-warning/90 font-medium">{probe.message}</p>
                  )}
                </div>
                {(() => {
                  const cliAcc = accounts.find((a) => a.id === probe.accountId);
                  if (!cliAcc) return null;
                  return (
                    <Button
                      size="xs"
                      variant="outline"
                      className="h-7 text-[11px] border-border shrink-0"
                      onClick={() => handleEdit(cliAcc, { alertsOnly: true })}
                    >
                      {t("providers.alertSettingsBtn")}
                    </Button>
                  );
                })()}
              </CardContent>
            </Card>
          ))}
        </div>
      </div>

      {/* ── Manual Accounts Section ────────────────────────────────── */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-bold text-muted-foreground uppercase tracking-wider">{t("providers.manual.section")}</h3>
          <div className="flex items-center gap-1.5">
            <Button
              variant="outline"
              size="xs"
              className="h-7 text-[11px] flex items-center gap-1.5 border-border"
              onClick={() => discoverMutation.mutate()}
              disabled={discoverMutation.isPending}
            >
              <Wand2 className={`h-3.5 w-3.5 ${discoverMutation.isPending ? "animate-spin" : ""}`} />
              {t("providers.manual.autoDiscover")}
            </Button>
            <Button size="xs" className="h-7 text-[11px] flex items-center gap-1.5" onClick={handleAddNew}>
              <Plus className="h-3.5 w-3.5" />
              {t("providers.manual.add")}
            </Button>
          </div>
        </div>

        {manualAccounts.length === 0 ? (
          <Empty
            title={t("providers.manual.emptyTitle")}
            description={t("providers.manual.emptyDesc")}
          />
        ) : (
          <div className="grid gap-2.5">
            {manualAccounts.map((acc) => (
              <Card key={acc.id} className="border-border bg-card/30 hover:bg-card/45 transition-colors shadow-sm">
                <CardContent className="p-3 flex items-center justify-between gap-3">
                  <div className="space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-semibold text-foreground">{acc.displayName}</span>
                      {!acc.enabled && <Badge variant="secondary" className="text-[9px] px-1 py-0 scale-90">{t("providers.manual.disabled")}</Badge>}
                    </div>
                    <p className="text-[10px] text-muted-foreground uppercase tracking-wider">
                      {t(`providers.provider.${acc.provider}`)} · {acc.config.type === "volcengine" ? t("providers.manual.sourceVolc") : t("providers.manual.sourceApi")}
                    </p>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted-foreground hover:text-foreground"
                      onClick={() => handleEdit(acc)}
                      aria-label={t("providers.manual.editAria", { name: acc.displayName })}
                    >
                      <Edit2 className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-status-danger hover:bg-status-danger/10"
                      onClick={() => setDeletingAccount(acc)}
                      aria-label={t("providers.manual.deleteAria", { name: acc.displayName })}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>

      {/* ── Add/Edit Dialog Form ──────────────────────────────────── */}
      {isFormOpen && (
        <Dialog open={isFormOpen} onOpenChange={(open) => {
          if (!open) resetForm();
          setIsFormOpen(open);
        }}>
          <DialogContent className="sm:max-w-[420px] bg-card border-border text-foreground overflow-y-auto max-h-[90vh]">
            <DialogHeader>
              <DialogTitle className="text-sm font-bold">
                {editingAccount
                  ? (alertsOnlyMode
                      ? t("providers.editAlertTitle", { name: editingAccount.displayName })
                      : t("providers.form.editTitle", { name: editingAccount.displayName }))
                  : t("providers.form.addTitle")}
              </DialogTitle>
              <DialogDescription className="text-xs text-muted-foreground">
                {alertsOnlyMode
                  ? t("providers.alertsFormDesc")
                  : t("providers.form.formDesc")}
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-3.5 mt-2 py-1">
              {!alertsOnlyMode && (
              <>
              {/* Type Select (Disabled on edit to prevent provider change) */}
              <Field label={t("providers.form.type")}>
                <Select
                  value={providerType}
                  onValueChange={(val) => { if (val) setProviderType(val as ProviderKind); }}
                  disabled={!!editingAccount || alertsOnlyMode}
                >
                  <SelectTrigger
                    aria-label={t("providers.form.type")}
                    className="w-full h-8 text-xs bg-card border-border text-foreground"
                  >
                    <SelectValue placeholder={t("providers.form.typePlaceholder")} />
                  </SelectTrigger>
                  <SelectContent className="bg-card border-border text-foreground">
                    <SelectItem value="kimi">{t("providers.provider.kimi")}</SelectItem>
                    <SelectItem value="zhipu">{t("providers.provider.zhipu")}</SelectItem>
                    <SelectItem value="zhipu_team">{t("providers.provider.zhipu_team")}</SelectItem>
                    <SelectItem value="minimax">{t("providers.provider.minimax")}</SelectItem>
                    <SelectItem value="zenmux">{t("providers.provider.zenmux")}</SelectItem>
                    <SelectItem value="volcengine">{t("providers.provider.volcengine")}</SelectItem>
                    <SelectItem value="copilot">{t("providers.provider.copilot")}</SelectItem>
                  </SelectContent>
                </Select>
              </Field>

              {/* Display Name */}
              <Field label={t("providers.form.name")}>
                <Input disabled={alertsOnlyMode}
                  aria-label={t("providers.form.name")}
                  className="h-8 text-xs bg-card border-border text-foreground"
                  placeholder={t("providers.form.namePlaceholder", { provider: t(`providers.provider.${providerType}`) })}
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                />
              </Field>

              {/* Copilot Device flow or Manual fields */}
              {providerType === "copilot" ? (
                <div className="space-y-3.5 border-t border-border/40 pt-3">
                  <Field label={t("providers.form.githubDomain")} description={t("providers.form.githubDomainDesc")}>
                    <Input
                      aria-label={t("providers.form.githubDomain")}
                      className="h-8 text-xs bg-card border-border text-foreground"
                      placeholder={t("providers.form.githubDomainPlaceholder")}
                      value={githubDomain}
                      onChange={(e) => setGithubDomain(e.target.value)}
                      disabled={copilotStatus === "polling"}
                    />
                  </Field>

                  {/* Device code actions */}
                  {copilotStatus === "idle" && (
                    <Button size="sm" className="w-full text-xs" onClick={handleCopilotLink}>
                      {t("providers.form.startLink")}
                    </Button>
                  )}

                  {copilotStatus === "requesting" && (
                    <div className="flex items-center justify-center gap-2 py-2">
                      <RefreshCw className="h-4 w-4 animate-spin text-muted-foreground" />
                      <span className="text-xs text-muted-foreground">{t("providers.form.requesting")}</span>
                    </div>
                  )}

                  {copilotStatus === "polling" && (
                    <div className="space-y-2 bg-muted/40 border border-border p-3 rounded-lg">
                      <div className="flex items-center justify-between">
                        <span className="text-[11px] text-muted-foreground font-semibold">{t("providers.form.deviceCode")}</span>
                        <div className="flex gap-1.5">
                          <Button size="xs" variant="outline" className="h-6 text-[10px] gap-1 border-border" onClick={handleCopyCode}>
                            <Copy className="h-3 w-3" /> {t("providers.form.copy")}
                          </Button>
                          <a
                            href={copilotVerifyUri}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center justify-center rounded-lg border border-border bg-background hover:bg-muted text-[10px] font-medium h-6 px-2 hover:text-foreground gap-1 select-none"
                          >
                            <ExternalLink className="h-3 w-3" /> {t("providers.form.openBrowser")}
                          </a>
                        </div>
                      </div>

                      <div className="text-center py-2.5 bg-card/65 border border-border rounded-md font-mono text-lg font-bold tracking-widest select-all text-foreground">
                        {copilotUserCode}
                      </div>

                      <div className="flex items-center justify-center gap-2 text-[10px] text-muted-foreground pt-1">
                        <RefreshCw className="h-3 w-3 animate-spin" />
                        <span>{t("providers.form.waiting")}</span>
                      </div>
                    </div>
                  )}

                  {copilotError && (
                    <p className="text-xs text-status-danger font-medium">{copilotError}</p>
                  )}
                </div>
              ) : (
                <>
                  {/* Zhipu Region */}
                  {providerType === "zhipu" && (
                    <Field label={t("providers.form.region")}>
                      <Select value={zhipuRegion} onValueChange={(val) => { if (val) setZhipuRegion(val); }}>
                        <SelectTrigger
                          aria-label={t("providers.form.region")}
                          className="w-full h-8 text-xs bg-card border-border text-foreground"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent className="bg-card border-border text-foreground">
                          <SelectItem value="cn">{t("providers.form.zhipuRegionCn")}</SelectItem>
                          <SelectItem value="global">{t("providers.form.zhipuRegionGlobal")}</SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  )}

                  {/* Zhipu Team Organization and Project IDs */}
                  {providerType === "zhipu_team" && (
                    <>
                      <Field label={t("providers.form.orgId")} description={t("providers.form.orgIdDesc")}>
                        <Input
                          aria-label={t("providers.form.orgId")}
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={zhipuTeamOrgId}
                          onChange={(e) => setZhipuTeamOrgId(e.target.value)}
                        />
                      </Field>
                      <Field label={t("providers.form.projectId")} description={t("providers.form.projectIdDesc")}>
                        <Input
                          aria-label={t("providers.form.projectId")}
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={zhipuTeamProjId}
                          onChange={(e) => setZhipuTeamProjId(e.target.value)}
                        />
                      </Field>
                    </>
                  )}

                  {/* MiniMax Region */}
                  {providerType === "minimax" && (
                    <Field label={t("providers.form.minimaxRegion")}>
                      <Select value={minimaxRegion} onValueChange={(val) => { if (val) setMinimaxRegion(val); }}>
                        <SelectTrigger
                          aria-label={t("providers.form.minimaxRegion")}
                          className="w-full h-8 text-xs bg-card border-border text-foreground"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent className="bg-card border-border text-foreground">
                          <SelectItem value="cn">{t("providers.form.minimaxRegionCn")}</SelectItem>
                          <SelectItem value="global">{t("providers.form.minimaxRegionGlobal")}</SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  )}

                  {/* ZenMux URL */}
                  {providerType === "zenmux" && (
                    <Field label={t("providers.form.zenmuxUrl")} description={t("providers.form.zenmuxUrlDesc")}>
                      <Input
                        aria-label={t("providers.form.zenmuxUrl")}
                        className="h-8 text-xs bg-card border-border text-foreground"
                        placeholder={t("providers.form.zenmuxUrlPlaceholder")}
                        value={zenmuxUrl}
                        onChange={(e) => setZenmuxUrl(e.target.value)}
                      />
                    </Field>
                  )}

                  {/* Volcengine Region */}
                  {providerType === "volcengine" && (
                    <Field label={t("providers.form.volcengineRegion")} description={t("providers.form.volcengineRegionDesc")}>
                      <Input
                        aria-label={t("providers.form.volcengineRegion")}
                        className="h-8 text-xs bg-card border-border text-foreground"
                        placeholder={t("providers.form.volcengineRegionPlaceholder")}
                        value={volcengineRegion}
                        onChange={(e) => setVolcengineRegion(e.target.value)}
                      />
                    </Field>
                  )}

                  {/* Volcengine AK / SK */}
                  {providerType === "volcengine" ? (
                    <div className="space-y-3">
                      <Field label={t("providers.form.accessKeyId")} description={editingAccount ? t("providers.form.keepExisting") : undefined}>
                        <Input
                          aria-label={t("providers.form.accessKeyId")}
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={accessKeyId}
                          onChange={(e) => setAccessKeyId(e.target.value)}
                        />
                      </Field>
                      <Field
                        label={t("providers.form.secretAccessKey")}
                        description={editingAccount ? t("providers.form.keepExisting") : undefined}
                      >
                        <div className="relative">
                          <Input
                            aria-label={t("providers.form.secretAccessKey")}
                            type={showSecret ? "text" : "password"}
                            className="h-8 text-xs bg-card border-border text-foreground pr-8"
                            value={secretAccessKey}
                            onChange={(e) => setSecretAccessKey(e.target.value)}
                          />
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="absolute right-1 top-1 h-6 w-6 text-muted-foreground"
                            onClick={() => setShowSecret(!showSecret)}
                            aria-label={showSecret ? t("providers.form.hideSecret") : t("providers.form.showSecret")}
                          >
                            {showSecret ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
                          </Button>
                        </div>
                      </Field>
                    </div>
                  ) : (
                    /* General API Key */
                    <Field label={t("providers.form.apiKey")} description={editingAccount ? t("providers.form.keepExisting") : undefined}>
                      <div className="relative">
                        <Input
                          aria-label={t("providers.form.apiKey")}
                          type={showSecret ? "text" : "password"}
                          className="h-8 text-xs bg-card border-border text-foreground pr-8"
                          placeholder={editingAccount ? "••••••••••••••••" : "xai-..."}
                          value={apiKey}
                          onChange={(e) => setApiKey(e.target.value)}
                        />
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="absolute right-1 top-1 h-6 w-6 text-muted-foreground"
                          onClick={() => setShowSecret(!showSecret)}
                          aria-label={showSecret ? t("providers.form.hideApiKey") : t("providers.form.showApiKey")}
                        >
                          {showSecret ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
                        </Button>
                      </div>
                    </Field>
                  )}
                </>
              )}

              </>
              )}

              {/* Quota Alerts */}
              <div className="border-t border-border/40 pt-3 space-y-2.5">
                <div className="space-y-0.5">
                  <span className="text-xs font-semibold text-foreground">{t("providers.alert.sectionTitle")}</span>
                  <p className="text-[10px] text-muted-foreground">
                    {t("providers.alert.sectionDesc")}
                  </p>
                </div>
                {!editingAccount ? (
                  <p className="text-[11px] text-muted-foreground">
                    {t("providers.alert.afterFirstRefresh")}
                  </p>
                ) : alertDrafts.length === 0 ? (
                  <p className="text-[11px] text-muted-foreground">
                    {t("providers.alert.afterFirstRefresh")}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {alertDrafts.map((rule) => {
                      const meta = alertMeta[rule.tierId];
                      const unlimited = !!meta?.unlimited;
                      const missing = !!meta?.missing;
                      const invalid =
                        !unlimited &&
                        !(
                          Number.isInteger(rule.thresholdPercent) &&
                          rule.thresholdPercent >= 1 &&
                          rule.thresholdPercent <= 99
                        );
                      return (
                        <div
                          key={rule.tierId}
                          className="rounded-md border border-border/50 bg-card/40 p-2.5 space-y-2"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <div className="space-y-0.5 min-w-0">
                              <div className="text-xs font-medium text-foreground truncate">
                                {meta?.label ?? rule.tierId}
                              </div>
                              {missing && (
                                <p className="text-[10px] text-status-warning">{t("providers.alert.missingTier")}</p>
                              )}
                              {unlimited && (
                                <p className="text-[10px] text-muted-foreground">{t("providers.alert.unlimitedNoAlert")}</p>
                              )}
                            </div>
                            <Switch
                              checked={unlimited ? false : rule.enabled}
                              disabled={unlimited}
                              onCheckedChange={(v) => handleAlertToggle(rule.tierId, v)}
                              aria-label={t("providers.alert.enableAria", { tier: meta?.label ?? rule.tierId })}
                            />
                          </div>
                          {!unlimited && (
                            <Field
                              label={t("providers.alert.thresholdLabel")}
                              description={invalid ? t("providers.alert.thresholdInvalid") : undefined}
                            >
                              <Input
                                aria-label={t("providers.alert.thresholdAria", { tier: meta?.label ?? rule.tierId })}
                                className="h-8 text-xs bg-card border-border text-foreground"
                                type="number"
                                min={1}
                                max={99}
                                step={1}
                                value={Number.isFinite(rule.thresholdPercent) ? rule.thresholdPercent : ""}
                                onChange={(e) => {
                                  const raw = e.target.value;
                                  const num = raw === "" ? Number.NaN : Number(raw);
                                  setAlertDrafts((prev) =>
                                    prev.map((r) =>
                                      r.tierId === rule.tierId
                                        ? { ...r, thresholdPercent: num }
                                        : r
                                    )
                                  );
                                }}
                              />
                            </Field>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>

              {/* Enabled Switch */}
              {!alertsOnlyMode && (
              <div className="flex items-center justify-between border-t border-border/40 pt-3 text-xs mt-1">
                <div className="space-y-0.5">
                  <span className="font-semibold text-foreground">{t("providers.form.enabled")}</span>
                  <p className="text-[10px] text-muted-foreground">{t("providers.form.enabledDesc")}</p>
                </div>
                <Switch
                  checked={enabled}
                  onCheckedChange={setEnabled}
                  aria-label={t("providers.form.enabled")}
                />
              </div>
              )}
            </div>

            <DialogFooter className="border-t border-border/40 pt-3 gap-2">
              <Button size="sm" variant="outline" className="text-xs h-8 border-border" onClick={() => { resetForm(); setIsFormOpen(false); }}>
                {t("providers.form.cancel")}
              </Button>
              {providerType !== "copilot" && (
                <Button size="sm" className="text-xs h-8" onClick={handleFormSubmit} disabled={saveMutation.isPending || (!!editingAccount && !alertRulesValid)}>
                  {t("providers.form.confirm")}
                </Button>
              )}
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}

      {/* ── Destructive Delete Confirmation ───────────────────────── */}
      {deletingAccount && (
        <AlertDialog open={!!deletingAccount} onOpenChange={(open) => {
          if (!open) setDeletingAccount(null);
        }}>
          <AlertDialogContent className="bg-card border-border text-foreground">
            <AlertDialogHeader>
              <AlertDialogTitle className="text-sm font-bold text-status-danger">
                {t("providers.delete.title")}
              </AlertDialogTitle>
              <AlertDialogDescription className="text-xs text-muted-foreground">
                {t("providers.delete.desc", { name: deletingAccount.displayName })}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter className="gap-2">
              <AlertDialogCancel className="text-xs h-8 border-border bg-transparent hover:bg-muted">
                {t("providers.delete.cancel")}
              </AlertDialogCancel>
              <AlertDialogAction
                className="text-xs h-8 bg-status-danger text-white hover:bg-status-danger/90"
                onClick={() => deleteMutation.mutate(deletingAccount.id)}
              >
                {t("providers.delete.confirm")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}
