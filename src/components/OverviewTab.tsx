import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  RefreshCw,
  Settings as SettingsIcon,
  AlertTriangle,
  CheckCircle,
  HelpCircle,
  Clock,
  TrendingUp,
  GripVertical,
  RotateCcw,
} from "lucide-react";
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
} from "recharts";

import { getSettings, getTierHistory, updateAccountOrder } from "@/lib/backend";
import { applyAccountOrder } from "@/lib/accountOrder";
import { getErrorMessage } from "@/lib/errors";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Empty } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { useDashboardSnapshot } from "@/hooks/useDashboardSnapshot";
import {
  QuotaTierList,
  formatAbsoluteQuota,
  formatTimeMargin,
  riskSeverity,
  sortTiers,
  statusColorClass,
  statusText,
  worstTier,
} from "@/components/QuotaTierList";
import type {
  AccountDashboard,
  ProviderKind,
  Settings,
  TierDashboard,
} from "@/types";

interface OverviewTabProps {
  onNavigateToSettings: () => void;
  onNavigateToProviders: () => void;
}

function providerLabel(p: ProviderKind): string {
  const labels: Record<ProviderKind, string> = {
    claude: "Claude",
    codex: "Codex",
    gemini: "Gemini",
    copilot: "Copilot",
    kimi: "Kimi",
    zhipu: "Zhipu",
    zhipu_team: "Zhipu Team",
    minimax: "MiniMax",
    zenmux: "ZenMux",
    volcengine: "Volcengine",
  };
  return labels[p] || p;
}

interface SelectedTier {
  account: AccountDashboard;
  tierId: string;
}

interface SortableAccountCardProps {
  account: AccountDashboard;
  selectedTierId: string | null;
  onSelectTier: (tier: TierDashboard) => void;
  onNavigateToProviders: () => void;
}

function SortableAccountCard({
  account: acc,
  selectedTierId,
  onSelectTier,
  onNavigateToProviders,
}: SortableAccountCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: acc.account.id });

  const hasError = acc.credentialStatus !== "valid" || !!acc.error;
  const w = worstTier(acc.tiers);
  const cardBorderClass = w
    ? statusColorClass(w.forecast.state).split(" ").pop() ?? "border-border"
    : "border-border";

  return (
    <Card
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      data-account-id={acc.account.id}
      className={`border bg-card/35 hover:bg-card/50 transition-colors shadow-sm overflow-hidden ${
        hasError ? "cursor-pointer" : ""
      } ${isDragging ? "opacity-80 z-10 shadow-md" : ""} ${cardBorderClass}`}
      onClick={() => {
        if (hasError) {
          onNavigateToProviders();
        }
      }}
    >
      <CardContent className="p-3.5 space-y-2.5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-start gap-1.5 min-w-0">
            <button
              type="button"
              ref={setActivatorNodeRef}
              className="mt-0.5 h-6 w-5 shrink-0 inline-flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-muted/60 cursor-grab active:cursor-grabbing touch-none"
              aria-label={`拖动排序 ${acc.account.displayName}`}
              onClick={(e) => e.stopPropagation()}
              {...attributes}
              {...listeners}
            >
              <GripVertical className="h-3.5 w-3.5" />
            </button>
            <div className="flex flex-col min-w-0">
              <span className="text-xs font-semibold text-foreground tracking-tight truncate">
                {acc.account.displayName}
              </span>
              <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
                {providerLabel(acc.account.provider)}
              </span>
            </div>
          </div>

          {hasError ? (
            <Badge variant="destructive" className="text-[10px] px-1.5 py-0 shrink-0">
              {acc.credentialStatus === "expired"
                ? "凭据过期"
                : acc.credentialStatus === "unavailable"
                ? "查询失败"
                : "配置错误"}
            </Badge>
          ) : w ? (
            <Badge
              variant="outline"
              className={`text-[10px] font-medium border px-1.5 py-0 shrink-0 ${statusColorClass(
                w.forecast.state
              )}`}
            >
              {statusText(w.forecast.state)}
            </Badge>
          ) : null}
        </div>

        {hasError ? (
          <p className="text-[10px] text-status-danger/90 font-medium leading-snug pl-6">
            {acc.error || "提供商凭证加载异常，请检查配置参数。"}
          </p>
        ) : acc.tiers.length > 0 ? (
          <div className="pl-6">
            <QuotaTierList
              tiers={acc.tiers}
              selectedTierId={selectedTierId}
              onSelectTier={onSelectTier}
            />
          </div>
        ) : (
          <p className="text-[10px] text-muted-foreground pl-6">
            暂无活跃配额监控数据，点击卡片添加。
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export function OverviewTab({ onNavigateToSettings, onNavigateToProviders }: OverviewTabProps) {
  const queryClient = useQueryClient();
  const { query, refreshMutation } = useDashboardSnapshot();
  const { data: dashboard, isLoading, isRefetching, isError, error } = query;
  const [selectedTier, setSelectedTier] = React.useState<SelectedTier | null>(null);

  // Resolve the active tier from (account, tierId). Falls back to first sorted
  // tier if the id vanished between snapshot and selection.
  const activeTier = React.useMemo<TierDashboard | null>(() => {
    if (!selectedTier) return null;
    const match = selectedTier.account.tiers.find((t) => t.quota.id === selectedTier.tierId);
    if (match) return match;
    const sorted = sortTiers(selectedTier.account.tiers);
    return sorted[0] ?? null;
  }, [selectedTier]);

  const { data: historyData, isLoading: isLoadingHistory } = useQuery({
    queryKey: [
      "history",
      selectedTier?.account.account.id,
      activeTier?.quota.id,
    ],
    queryFn: () => {
      if (!selectedTier || !activeTier) {
        return Promise.resolve([]);
      }
      return getTierHistory(
        selectedTier.account.account.id,
        activeTier.quota.id,
        24
      );
    },
    enabled: !!selectedTier && !!activeTier,
  });

  const handleManualRefresh = React.useCallback(() => {
    if (refreshMutation.isPending || isRefetching) return;
    refreshMutation.mutate();
  }, [refreshMutation, isRefetching]);

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  const orderMutation = useMutation({
    mutationFn: updateAccountOrder,
    onSuccess: (_data, order) => {
      queryClient.setQueryData(["settings"], (prev: Settings | undefined) =>
        prev
          ? { ...prev, accountOrder: order }
          : { refreshIntervalMinutes: 5, accountOrder: order },
      );
    },
  });

  // Manual order wins when present; otherwise sort by risk severity.
  // While settings are still loading, trust the backend-ordered snapshot so we
  // do not briefly re-sort by risk and diverge from tray/native menu.
  const sortedAccounts = React.useMemo<AccountDashboard[]>(() => {
    if (!dashboard) return [];
    if (settings === undefined) return dashboard.accounts;
    return applyAccountOrder(dashboard.accounts, settings.accountOrder);
  }, [dashboard, settings]);

  const hasCustomOrder = (settings?.accountOrder?.length ?? 0) > 0;

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = React.useCallback(
    (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;
      const ids = sortedAccounts.map((acc) => acc.account.id);
      const oldIndex = ids.indexOf(String(active.id));
      const newIndex = ids.indexOf(String(over.id));
      if (oldIndex < 0 || newIndex < 0) return;
      const nextOrder = arrayMove(ids, oldIndex, newIndex);
      queryClient.setQueryData(["settings"], (prev: Settings | undefined) =>
        prev
          ? { ...prev, accountOrder: nextOrder }
          : { refreshIntervalMinutes: 5, accountOrder: nextOrder },
      );
      orderMutation.mutate(nextOrder);
    },
    [sortedAccounts, queryClient, orderMutation],
  );

  const handleResetOrder = React.useCallback(() => {
    queryClient.setQueryData(["settings"], (prev: Settings | undefined) =>
      prev
        ? { ...prev, accountOrder: [] }
        : { refreshIntervalMinutes: 5, accountOrder: [] },
    );
    orderMutation.mutate([]);
  }, [queryClient, orderMutation]);

  // Find worst risk details for RiskHero
  const riskHeroInfo = React.useMemo(() => {
    if (!dashboard || dashboard.accounts.length === 0) return null;

    let worstTierRef: { account: AccountDashboard; tier: TierDashboard } | null = null;
    let worstSeverity = -1;

    for (const acc of dashboard.accounts) {
      if (acc.credentialStatus !== "valid" || acc.error) {
        const errSev = riskSeverity("unknown_reset") + 0.5;
        if (errSev > worstSeverity) {
          worstSeverity = errSev;
          worstTierRef = null;
        }
        continue;
      }
      for (const tier of acc.tiers) {
        const sev = riskSeverity(tier.forecast.state);
        if (sev > worstSeverity) {
          worstSeverity = sev;
          worstTierRef = { account: acc, tier };
        } else if (sev === worstSeverity && worstTierRef) {
          if (tier.quota.utilization > worstTierRef.tier.quota.utilization) {
            worstTierRef = { account: acc, tier };
          } else if (
            tier.quota.utilization === worstTierRef.tier.quota.utilization &&
            (tier.forecast.exhaustionAt ?? tier.quota.resetsAt ?? Number.POSITIVE_INFINITY) <
              (worstTierRef.tier.forecast.exhaustionAt ??
                worstTierRef.tier.quota.resetsAt ??
                Number.POSITIVE_INFINITY)
          ) {
            worstTierRef = { account: acc, tier };
          }
        }
      }
    }

    return { worstTier: worstTierRef, worstSeverity };
  }, [dashboard]);

  // Loading skeleton
  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-[120px] w-full rounded-lg" />
        <div className="space-y-2">
          <Skeleton className="h-[100px] w-full rounded-lg" />
          <Skeleton className="h-[100px] w-full rounded-lg" />
          <Skeleton className="h-[100px] w-full rounded-lg" />
        </div>
      </div>
    );
  }

  const hasAccounts = dashboard && dashboard.accounts.length > 0;
  const isRefreshing = refreshMutation.isPending || isRefetching;

  return (
    <div className="space-y-4">
      {/* ── Header Toolbar ────────────────────────────────────────── */}
      <div className="flex items-center justify-between">
        <div className="flex flex-col">
          <h2 className="text-sm font-semibold text-muted-foreground tracking-tight">状态面板</h2>
          {dashboard && (
            <p className="text-[10px] text-muted-foreground">
              最近刷新: {new Date(dashboard.refreshedAt).toLocaleTimeString()}
            </p>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          {hasCustomOrder && (
            <Button
              variant="outline"
              size="sm"
              className="h-8 px-2 text-[11px] text-muted-foreground border-border hover:bg-muted"
              onClick={handleResetOrder}
              disabled={orderMutation.isPending}
              aria-label="恢复风险排序"
            >
              <RotateCcw className="h-3 w-3 mr-1" />
              恢复风险排序
            </Button>
          )}
          <Button
            variant="outline"
            size="icon"
            className="h-8 w-8 text-muted-foreground border-border hover:bg-muted"
            onClick={handleManualRefresh}
            disabled={isRefreshing}
            aria-label="刷新额度"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${isRefreshing ? "animate-spin" : ""}`} />
          </Button>
          <Button
            variant="outline"
            size="icon"
            className="h-8 w-8 text-muted-foreground border-border hover:bg-muted"
            onClick={onNavigateToSettings}
            aria-label="设置"
          >
            <SettingsIcon className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      {/* ── Risk Hero Panel ───────────────────────────────────────── */}
      {hasAccounts && riskHeroInfo && (
        <Card className="border-border bg-card/45 shadow-sm overflow-hidden">
          <CardContent className="p-4 flex items-start gap-3">
            {riskHeroInfo.worstSeverity >= 4 ? (
              <div className="p-2.5 rounded-full bg-status-danger/10 text-status-danger border border-status-danger/15 self-start">
                <AlertTriangle className="h-5 w-5" />
              </div>
            ) : riskHeroInfo.worstSeverity >= 3 ? (
              <div className="p-2.5 rounded-full bg-secondary text-muted-foreground border border-border self-start">
                <HelpCircle className="h-5 w-5" />
              </div>
            ) : riskHeroInfo.worstSeverity >= 1 ? (
              <div className="p-2.5 rounded-full bg-status-stale/10 text-status-stale border border-status-stale/15 self-start">
                <Clock className="h-5 w-5" />
              </div>
            ) : (
              <div className="p-2.5 rounded-full bg-status-safe/10 text-status-safe border border-status-safe/15 self-start">
                <CheckCircle className="h-5 w-5" />
              </div>
            )}

            <div className="flex-1 space-y-1">
              <h3 className="text-xs font-semibold text-muted-foreground tracking-wider uppercase">最早风险预警</h3>
              {riskHeroInfo.worstTier ? (
                <div>
                  <p className="text-sm font-bold text-foreground">
                    {riskHeroInfo.worstTier.account.account.displayName} · {riskHeroInfo.worstTier.tier.quota.label}
                  </p>
                  <p className="text-xs text-muted-foreground mt-0.5">
                    {riskHeroInfo.worstTier.tier.forecast.state === "exhausted" ? (
                      <span className="text-status-danger font-medium">额度已耗尽</span>
                    ) : riskHeroInfo.worstTier.tier.forecast.state === "at_risk" ? (
                      <span className="text-status-warning font-medium">
                        预计在 {formatTimeMargin(riskHeroInfo.worstTier.tier.forecast.exhaustionAt)} 后耗尽，领先重置期限。
                      </span>
                    ) : riskHeroInfo.worstTier.tier.forecast.state === "learning" ? (
                      <span>正在收集样本，计算消耗速率。</span>
                    ) : riskHeroInfo.worstTier.tier.quota.unlimited ? (
                      <span>套餐状态安全，无耗尽风险。</span>
                    ) : (
                      <span>套餐状态安全，速率稳定。</span>
                    )}
                  </p>
                </div>
              ) : riskHeroInfo.worstSeverity >= 3 ? (
                <div>
                  <p className="text-sm font-bold text-foreground">提供商凭据异常</p>
                  <p className="text-xs text-muted-foreground mt-0.5">有提供商的 API 密钥或凭据已过期或校验失败。</p>
                </div>
              ) : (
                <div>
                  <p className="text-sm font-bold text-foreground">所有套餐状态安全</p>
                  <p className="text-xs text-muted-foreground mt-0.5">近期的消耗速率显示，所有套餐均可支撑至重置期。</p>
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* ── Dashboard Query Error ───────────────────────────────── */}
      {isError && !dashboard && (
        <Card className="border-status-danger/40 bg-status-danger/5 shadow-sm">
          <CardContent className="p-4 flex items-start gap-3">
            <div className="p-2 rounded-full bg-status-danger/10 text-status-danger border border-status-danger/20 self-start">
              <AlertTriangle className="h-4 w-4" />
            </div>
            <div className="flex-1 space-y-1.5">
              <h3 className="text-xs font-semibold text-foreground">加载仪表盘失败</h3>
              <p className="text-xs text-muted-foreground leading-snug">
                {getErrorMessage(error) || "无法连接到本地服务，请稍后重试。"}
              </p>
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-xs border-status-danger/30 text-status-danger hover:bg-status-danger/10 mt-1"
                onClick={handleManualRefresh}
                disabled={isRefreshing}
              >
                <RefreshCw className={`h-3 w-3 mr-1 ${isRefreshing ? "animate-spin" : ""}`} />
                重试
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {/* ── Empty State ───────────────────────────────────────────── */}
      {!hasAccounts && !isError && (
        <Empty
          icon={<AlertTriangle className="h-8 w-8" />}
          title="暂无启用的提供商"
          description="点击下方按钮，配置你的第一个 LLM 凭据以实时监控配额。"
          action={
            <Button size="sm" onClick={onNavigateToProviders}>
              添加提供商
            </Button>
          }
        />
      )}

      {/* ── Account Cards List ────────────────────────────────────── */}
      {hasAccounts && (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext
            items={sortedAccounts.map((acc) => acc.account.id)}
            strategy={verticalListSortingStrategy}
          >
            <div className="space-y-3">
              {sortedAccounts.map((acc) => (
                <SortableAccountCard
                  key={acc.account.id}
                  account={acc}
                  selectedTierId={
                    selectedTier?.account.account.id === acc.account.id
                      ? selectedTier.tierId
                      : null
                  }
                  onSelectTier={(tier) =>
                    setSelectedTier({ account: acc, tierId: tier.quota.id })
                  }
                  onNavigateToProviders={onNavigateToProviders}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}

      {/* ── Details Dialog & Chart ────────────────────────────────── */}
      {selectedTier && activeTier && (
        <Dialog
          open={!!selectedTier}
          onOpenChange={(open) => {
            if (!open) setSelectedTier(null);
          }}
        >
          <DialogContent className="sm:max-w-[420px] bg-card border-border text-foreground">
            <DialogHeader className="space-y-1.5">
              <DialogTitle className="text-sm font-bold flex items-center justify-between gap-2">
                <span>
                  {selectedTier.account.account.displayName} · {activeTier.quota.label}
                </span>
                <Badge
                  variant="outline"
                  className={`text-[10px] border px-1.5 py-0 ${statusColorClass(
                    activeTier.forecast.state
                  )}`}
                >
                  {statusText(activeTier.forecast.state)}
                </Badge>
              </DialogTitle>
              <DialogDescription className="text-xs text-muted-foreground">
                当前周期配额的实时消耗速率及耗尽风险预估。
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-4 mt-2">
              {/* Tier switcher */}
              {selectedTier.account.tiers.length > 1 && (
                <div className="flex flex-wrap gap-1.5">
                  {sortTiers(selectedTier.account.tiers).map((t) => {
                    const isActive = t.quota.id === activeTier.quota.id;
                    return (
                      <button
                        key={t.quota.id}
                        type="button"
                        onClick={() =>
                          setSelectedTier({ account: selectedTier.account, tierId: t.quota.id })
                        }
                        className={`px-2 py-1 rounded-md border text-[10px] font-medium transition-colors ${
                          isActive
                            ? "border-foreground/50 bg-secondary text-foreground"
                            : "border-border/60 bg-card/30 text-muted-foreground hover:bg-card/55"
                        }`}
                        data-tier-switch-id={t.quota.id}
                        data-active={isActive ? "true" : "false"}
                      >
                        {t.quota.label}
                      </button>
                    );
                  })}
                </div>
              )}

              {/* Stats Grid */}
              <div className="grid grid-cols-2 gap-3 text-xs">
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">窗口类型</span>
                  <span className="font-semibold text-foreground">{activeTier.quota.label}</span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">当前已用</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.quota.unlimited ? "无限额度" : `${activeTier.quota.utilization.toFixed(1)}%`}
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">绝对额度</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.quota.unlimited
                      ? "无限额度"
                      : formatAbsoluteQuota(activeTier.quota) ?? "—"}
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">消耗速度</span>
                  <span className="font-semibold text-foreground flex items-center gap-1">
                    {activeTier.quota.unlimited ? (
                      <span>—</span>
                    ) : (
                      <>
                        <TrendingUp className="h-3 w-3 text-status-warning" />
                        {activeTier.forecast.ratePerHour.toFixed(1)}%/时
                      </>
                    )}
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">重置周期</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.quota.resetsAt
                      ? formatTimeMargin(activeTier.quota.resetsAt)
                      : "未知"}
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">预计重置时使用率</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.quota.unlimited
                      ? "—"
                      : `${activeTier.forecast.projectedUtilizationAtReset.toFixed(1)}%`}
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">样本数</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.forecast.sampleCount} 个样本 · 观察{" "}
                    {activeTier.forecast.observationMinutes} 分钟
                  </span>
                </div>
                <div className="border border-border/50 rounded-lg p-2.5 bg-card/20 space-y-0.5">
                  <span className="text-[10px] text-muted-foreground block">预计耗尽</span>
                  <span className="font-semibold text-foreground">
                    {activeTier.quota.unlimited
                      ? "—"
                      : activeTier.forecast.exhaustionAt
                      ? new Date(activeTier.forecast.exhaustionAt).toLocaleString()
                      : "—"}
                  </span>
                </div>
              </div>

              {/* Linear Forecast Info */}
              {activeTier.forecast.state === "at_risk" && (
                <div className="border border-status-danger/10 bg-status-danger/5 rounded-lg p-2.5 text-xs text-status-danger space-y-1">
                  <p className="font-semibold flex items-center gap-1">
                    <AlertTriangle className="h-3.5 w-3.5" /> 存在早于重置期的额度耗尽风险
                  </p>
                  <p className="text-[10px] text-status-danger/80">
                    按照目前 {activeTier.forecast.ratePerHour.toFixed(1)}%/小时 的消耗速度，额度将在{" "}
                    {activeTier.forecast.exhaustionAt
                      ? new Date(activeTier.forecast.exhaustionAt).toLocaleTimeString()
                      : "未知"}
                    （约 {formatTimeMargin(activeTier.forecast.exhaustionAt)} 后）耗尽，而重置发生在此之后。
                  </p>
                </div>
              )}

              {/* Utilization Line Chart */}
              <div className="space-y-1.5">
                <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-wider block">
                  近 24 小时已用额度趋势
                </span>

                {isLoadingHistory ? (
                  <Skeleton className="h-[140px] w-full rounded-lg" />
                ) : historyData && historyData.length > 0 ? (
                  <div className="h-[140px] w-full border border-border/50 rounded-lg p-2 bg-card/10">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart
                        data={historyData}
                        margin={{ top: 5, right: 5, left: -25, bottom: 5 }}
                      >
                        <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" opacity={0.3} />
                        <XAxis
                          dataKey="sampledAt"
                          stroke="var(--muted-foreground)"
                          fontSize={9}
                          tickLine={false}
                          axisLine={false}
                          tickFormatter={(t) =>
                            new Date(t).toLocaleTimeString([], {
                              hour: "2-digit",
                              minute: "2-digit",
                            })
                          }
                        />
                        <YAxis
                          stroke="var(--muted-foreground)"
                          fontSize={9}
                          tickLine={false}
                          axisLine={false}
                          domain={[0, 100]}
                        />
                        <Line
                          type="monotone"
                          dataKey="utilization"
                          name="已用比例"
                          stroke={
                            activeTier.forecast.state === "exhausted" ||
                            activeTier.quota.utilization >= 90
                              ? "var(--status-danger)"
                              : activeTier.forecast.state === "at_risk" ||
                                activeTier.quota.utilization >= 70
                              ? "var(--status-warning)"
                              : "var(--status-safe)"
                          }
                          strokeWidth={2}
                          dot={false}
                        />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                ) : (
                  <Empty
                    title="暂无历史趋势数据"
                    description="开始消耗额度后，历史用量柱/线图将在此展示。"
                    className="h-[140px] py-4"
                  />
                )}
              </div>
            </div>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}
