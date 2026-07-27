import * as React from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { Info, Shield, KeyRound, Trash2 } from "lucide-react";

import {
  getSettings,
  updateSettings,
  clearHistory,
} from "@/lib/backend";
import { getErrorMessage } from "@/lib/errors";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
import { Field } from "@/components/ui/field";

export function SettingsTab() {
  const queryClient = useQueryClient();
  const [isClearOpen, setIsClearOpen] = React.useState(false);

  // 1. Fetch settings
  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  // 2. Update Settings Mutation
  const updateSettingsMutation = useMutation({
    mutationFn: updateSettings,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      toast.success("设置更新成功");
    },
    onError: (err) => {
      toast.error(`设置更新失败: ${getErrorMessage(err)}`);
    },
  });

  // 3. Clear History Mutation
  const clearHistoryMutation = useMutation({
    mutationFn: clearHistory,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setIsClearOpen(false);
      toast.success("历史记录及预测数据已清空");
    },
    onError: (err) => {
      toast.error(`清空历史失败: ${getErrorMessage(err)}`);
    },
  });

  const handleIntervalChange = (val: string | null) => {
    if (!settings || !val) return;
    const mins = parseInt(val, 10);
    if (![5, 10, 15, 30].includes(mins)) {
      toast.error("非法的刷新频率选项");
      return;
    }
    updateSettingsMutation.mutate({
      refreshIntervalMinutes: mins,
    });
  };

  return (
    <div className="space-y-4">
      {/* ── Behavior Settings ─────────────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">通用行为</CardTitle>
          <CardDescription className="text-[10px] text-muted-foreground">配置配额轮询刷新的工作频率。</CardDescription>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3.5">
          <Field label="自动后台刷新频率" description="后台调度更新的周期。缩短周期可更实时地捕获变化，但会略微增加请求频次。">
            <Select
              value={(settings?.refreshIntervalMinutes ?? 5).toString()}
              onValueChange={handleIntervalChange}
              disabled={updateSettingsMutation.isPending}
            >
              <SelectTrigger
                aria-label="自动后台刷新频率"
                className="w-full h-8 text-xs bg-card border-border text-foreground"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="bg-card border-border text-foreground">
                <SelectItem value="5">每 5 分钟 (默认)</SelectItem>
                <SelectItem value="10">每 10 分钟</SelectItem>
                <SelectItem value="15">每 15 分钟</SelectItem>
                <SelectItem value="30">每 30 分钟</SelectItem>
              </SelectContent>
            </Select>
          </Field>
        </CardContent>
      </Card>

      {/* ── Data & History Management ────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">数据与历史记录</CardTitle>
          <CardDescription className="text-[10px] text-muted-foreground">管理本地历史快照数据。</CardDescription>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3">
          <div className="flex items-start gap-2 text-xs">
            <Info className="h-4 w-4 text-muted-foreground mt-0.5" />
            <div className="space-y-0.5 flex-1">
              <span className="font-semibold text-foreground">历史快照自动清理</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                为了避免磁盘占用膨胀，本地 SQLite 数据库会自动保留最近 30 天的额度快照数据，每天首次更新后会自动修剪过期行。
              </p>
            </div>
          </div>

          <div className="flex items-center justify-between border-t border-border/40 pt-3 text-xs mt-1">
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">清空用量历史</span>
              <p className="text-[10px] text-muted-foreground">删除所有保存在本地的历史快照。耗尽预测模型将重置并重新开始学习。</p>
            </div>
            <Button
              variant="outline"
              size="sm"
              className="text-xs h-8 border-status-danger text-status-danger hover:bg-status-danger/10 hover:text-status-danger"
              onClick={() => setIsClearOpen(true)}
            >
              <Trash2 className="h-3.5 w-3.5 mr-1" /> 清空历史
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* ── Security Disclosures ─────────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">凭据与安全说明</CardTitle>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3 text-xs">
          <div className="flex items-start gap-2.5">
            <Shield className="h-4 w-4 text-status-safe mt-0.5" />
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">环境变量读取</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                敏感密钥优先从 shell 环境变量读取（如 ANTHROPIC_API_KEY、GITHUB_TOKEN 等），运行时仅将 OAuth 设备等动态令牌缓存在内存中。本地 SQLite 数据库仅包含显示名称、启用状态和非敏感的区域设置。
              </p>
            </div>
          </div>
          
          <div className="flex items-start gap-2.5">
            <KeyRound className="h-4 w-4 text-muted-foreground mt-0.5" />
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">无外部中转与明文泄露</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                Last Token 绝不会上传你的 API 密钥到任何外部服务器。所有 API 用量请求均直接从你的本地客户端安全发送给对应的服务提供商。
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* ── App Version ───────────────────────────────────────────── */}
      <div className="text-center text-[10px] text-muted-foreground py-2 tracking-wider">
        Last Token · 版本 0.1.0
      </div>

      {/* ── Clear History Confirmation ────────────────────────────── */}
      {isClearOpen && (
        <AlertDialog open={isClearOpen} onOpenChange={setIsClearOpen}>
          <AlertDialogContent className="bg-card border-border text-foreground">
            <AlertDialogHeader>
              <AlertDialogTitle className="text-sm font-bold text-status-danger flex items-center gap-1.5">
                <Trash2 className="h-4 w-4" /> 确定要清空预测历史吗？
              </AlertDialogTitle>
              <AlertDialogDescription className="text-xs text-muted-foreground">
                该操作将清除本地存储的所有额度百分比快照。耗尽速率预测模型将重置为“正在分析消耗速度...”状态，直至累积 3 个新的快照样本。本操作无法撤销。
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter className="gap-2">
              <AlertDialogCancel className="text-xs h-8 border-border bg-transparent hover:bg-muted">
                取消
              </AlertDialogCancel>
              <AlertDialogAction
                className="text-xs h-8 bg-status-danger text-white hover:bg-status-danger/90"
                onClick={() => clearHistoryMutation.mutate()}
                disabled={clearHistoryMutation.isPending}
              >
                确定清空
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}
