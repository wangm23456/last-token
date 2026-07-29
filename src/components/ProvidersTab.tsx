import * as React from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  Plus,
  Trash2,
  Edit2,
  RefreshCw,
  Info,
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
} from "@/lib/backend";
import { applyAccountOrder, applyIdOrder } from "@/lib/accountOrder";
import { getErrorMessage } from "@/lib/errors";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
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
import { PublicAccount, ProviderConfig, ProviderKind, SecretPayload, CredentialProbe } from "@/types";

const providerLabel = (p: ProviderKind) => {
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
};

export function ProvidersTab() {
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
      toast.success("提供商保存成功");
    },
    onError: (err) => {
      toast.error(`保存失败: ${getErrorMessage(err)}`);
    },
  });

  // 4. Delete Account Mutation
  const deleteMutation = useMutation({
    mutationFn: deleteAccount,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setDeletingAccount(null);
      toast.success("提供商删除成功");
    },
    onError: (err) => {
      toast.error(`删除失败: ${getErrorMessage(err)}`);
    },
  });

  // 5. Discover Env Accounts Mutation
  const discoverMutation = useMutation({
    mutationFn: discoverEnvAccounts,
    onSuccess: (created) => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      if (created.length === 0) {
        toast.info("未发现新的环境变量凭证");
      } else {
        toast.success(`已自动添加 ${created.length} 个提供商：${created.map((a) => a.displayName).join(", ")}`);
      }
    },
    onError: (err) => {
      toast.error(`自动发现失败: ${getErrorMessage(err)}`);
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
          parts.push(`官方 CLI 凭证 (${newCli.map((c) => providerLabel(c.provider)).join(", ")})`);
        }
        if (envCount > 0) {
          parts.push(`环境变量提供商 (${envAccounts.map((e) => e.displayName).join(", ")})`);
        }
        toast.success(`导入成功！自动添加了：${parts.join(" 和 ")}`);
      } else {
        toast.info("未检测到新的凭证（可能已导入，或相应环境变量/CLI 配置文件不存在）。");
      }
    } catch (err) {
      toast.error(`检测与导入失败: ${getErrorMessage(err)}`);
    } finally {
      setIsImporting(false);
    }
  };

  const handleEdit = (acc: PublicAccount) => {
    setEditingAccount(acc);
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

    setIsFormOpen(true);
  };

  const handleAddNew = () => {
    setEditingAccount(null);
    resetForm();
    setIsFormOpen(true);
  };

  const resetForm = () => {
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

  const handleFormSubmit = () => {
    if (!displayName.trim()) {
      toast.error("请输入提供商名称");
      return;
    }

    // ZenMux URL Validation
    if (providerType === "zenmux") {
      if (!zenmuxUrl.startsWith("https://")) {
        toast.error("ZenMux 额度 URL 必须以 https:// 开头");
        return;
      }
      if (!zenmuxUrl.includes("zenmux.")) {
        toast.error("ZenMux 额度 URL 必须包含 zenmux. 域名");
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
          toast.error("请输入 Org ID 和 Project ID");
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
        toast.success("用户验证码已复制到剪贴板，请在浏览器中粘贴激活。");
      } catch {
        toast.info("无法自动复制验证码，请手动复制。");
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
              toast.success("GitHub Copilot 关联成功");
              queryClient.invalidateQueries({ queryKey: ["accounts"] });
              queryClient.invalidateQueries({ queryKey: ["dashboard"] });
              resetForm();
              setIsFormOpen(false);
            } else if (pollRes.type === "expired") {
              stopPolling();
              setCopilotStatus("expired");
              setCopilotError("验证码已过期，请重新获取。");
            } else if (pollRes.type === "denied") {
              stopPolling();
              setCopilotStatus("error");
              setCopilotError(pollRes.message || "用户取消了授权。");
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
              err instanceof Error ? err.message : "轮询出错",
            );
          }
        }, sec * 1000);
      };
      startPolling(intervalSec);
    } catch (err) {
      setCopilotStatus("error");
      setCopilotError(err instanceof Error ? err.message : "请求设备码失败");
    }
  };

  const handleCopyCode = () => {
    navigator.clipboard
      .writeText(copilotUserCode)
      .then(() => toast.success("验证码已复制"))
      .catch(() => toast.error("复制失败，请手动选中复制。"));
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
        return <Badge variant="outline" className="text-[10px] text-status-safe border-status-safe/20 bg-status-safe/5">已检测</Badge>;
      case "expired":
        return <Badge variant="outline" className="text-[10px] text-status-warning border-status-warning/20 bg-status-warning/5">已过期</Badge>;
      case "not_found":
        return <Badge variant="outline" className="text-[10px] text-muted-foreground">未检测到</Badge>;
      default:
        return <Badge variant="outline" className="text-[10px] text-muted-foreground">检测异常</Badge>;
    }
  };

  const getCliSourceGuide = (provider: string) => {
    switch (provider) {
      case "claude":
        return "尝试读取配置文件 ~/.claude/.credentials.json";
      case "codex":
        return "尝试读取配置文件 ~/.codex/auth.json (需为 chatgpt/OAuth 模式)";
      case "gemini":
        return "尝试读取配置文件 ~/.gemini/oauth_creds.json";
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
            <h4 className="text-xs font-semibold text-foreground">一键导入凭证</h4>
            <p className="text-[10px] text-muted-foreground">
              检测本地官方 CLI 配置文件与当前环境变量，自动导入所有可用的 API 密钥和账户凭证。
            </p>
          </div>
          <Button
            size="sm"
            className="h-8 text-xs flex items-center gap-1.5 shadow-sm"
            onClick={handleOneClickImport}
            disabled={isImporting}
          >
            <Wand2 className={`h-3.5 w-3.5 ${isImporting ? "animate-spin" : ""}`} />
            检测并导入
          </Button>
        </CardContent>
      </Card>

      {/* ── CLI Auto Discovery Section ────────────────────────────── */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-bold text-muted-foreground uppercase tracking-wider">官方 CLI 自动扫描</h3>
          <Button
            variant="ghost"
            size="xs"
            className="text-[11px] text-muted-foreground hover:bg-muted h-7 flex items-center gap-1"
            onClick={() => refetchProbes()}
            disabled={isProbing}
          >
            <RefreshCw className={`h-3 w-3 ${isProbing ? "animate-spin" : ""}`} />
            重新扫描
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
                      {probe.provider === "claude" ? "Claude Code" : probe.provider === "codex" ? "Codex CLI" : "Gemini CLI"}
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
              </CardContent>
            </Card>
          ))}
        </div>
      </div>

      {/* ── Manual Accounts Section ────────────────────────────────── */}
      <div className="space-y-2.5">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-bold text-muted-foreground uppercase tracking-wider">自管/手动提供商</h3>
          <div className="flex items-center gap-1.5">
            <Button
              variant="outline"
              size="xs"
              className="h-7 text-[11px] flex items-center gap-1.5 border-border"
              onClick={() => discoverMutation.mutate()}
              disabled={discoverMutation.isPending}
            >
              <Wand2 className={`h-3.5 w-3.5 ${discoverMutation.isPending ? "animate-spin" : ""}`} />
              自动发现
            </Button>
            <Button size="xs" className="h-7 text-[11px] flex items-center gap-1.5" onClick={handleAddNew}>
              <Plus className="h-3.5 w-3.5" />
              添加提供商
            </Button>
          </div>
        </div>

        {manualAccounts.length === 0 ? (
          <Empty
            title="暂无手动配置的提供商"
            description="点击上方“自动发现”可从环境变量批量检测添加（如 MOONSHOT_API_KEY / KIMI_API_KEY / MINIMAX_API_KEY 等）；或手动添加提供商。"
          />
        ) : (
          <div className="grid gap-2.5">
            {manualAccounts.map((acc) => (
              <Card key={acc.id} className="border-border bg-card/30 hover:bg-card/45 transition-colors shadow-sm">
                <CardContent className="p-3 flex items-center justify-between gap-3">
                  <div className="space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-semibold text-foreground">{acc.displayName}</span>
                      {!acc.enabled && <Badge variant="secondary" className="text-[9px] px-1 py-0 scale-90">已禁用</Badge>}
                    </div>
                    <p className="text-[10px] text-muted-foreground uppercase tracking-wider">
                      {providerLabel(acc.provider)} · {acc.config.type === "volcengine" ? "AK/SK 环境变量" : "API 环境变量"}
                    </p>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted-foreground hover:text-foreground"
                      onClick={() => handleEdit(acc)}
                      aria-label={`编辑 ${acc.displayName}`}
                    >
                      <Edit2 className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-status-danger hover:bg-status-danger/10"
                      onClick={() => setDeletingAccount(acc)}
                      aria-label={`删除 ${acc.displayName}`}
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
                {editingAccount ? `编辑提供商: ${editingAccount.displayName}` : "添加自管提供商"}
              </DialogTitle>
              <DialogDescription className="text-xs text-muted-foreground">
                凭证优先从对应环境变量读取（如 KIMI_API_KEY、MINIMAX_API_KEY、ANTHROPIC_API_KEY、GITHUB_TOKEN 等）。下方输入仅在当前会话内存缓存，不会持久化到钥匙串或磁盘。
              </DialogDescription>
            </DialogHeader>

            <div className="space-y-3.5 mt-2 py-1">
              {/* Type Select (Disabled on edit to prevent provider change) */}
              <Field label="提供商类型">
                <Select
                  value={providerType}
                  onValueChange={(val) => { if (val) setProviderType(val as ProviderKind); }}
                  disabled={!!editingAccount}
                >
                  <SelectTrigger
                    aria-label="提供商类型"
                    className="w-full h-8 text-xs bg-card border-border text-foreground"
                  >
                    <SelectValue placeholder="选择提供商" />
                  </SelectTrigger>
                  <SelectContent className="bg-card border-border text-foreground">
                    <SelectItem value="kimi">Kimi For Coding</SelectItem>
                    <SelectItem value="zhipu">智谱 GLM (个人版)</SelectItem>
                    <SelectItem value="zhipu_team">智谱 GLM (团队版)</SelectItem>
                    <SelectItem value="minimax">MiniMax 编程套餐</SelectItem>
                    <SelectItem value="zenmux">ZenMux 代理配额</SelectItem>
                    <SelectItem value="volcengine">火山方舟 Ark (OpenAPI)</SelectItem>
                    <SelectItem value="copilot">GitHub Copilot</SelectItem>
                  </SelectContent>
                </Select>
              </Field>

              {/* Display Name */}
              <Field label="显示名称">
                <Input
                  aria-label="显示名称"
                  className="h-8 text-xs bg-card border-border text-foreground"
                  placeholder={`如: My ${providerLabel(providerType)} Account`}
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                />
              </Field>

              {/* Copilot Device flow or Manual fields */}
              {providerType === "copilot" ? (
                <div className="space-y-3.5 border-t border-border/40 pt-3">
                  <Field label="GitHub 域名 (可选)" description="企业版 GHES 实例需要填入完整的主机域名。默认使用 github.com。">
                    <Input
                      aria-label="GitHub 域名"
                      className="h-8 text-xs bg-card border-border text-foreground"
                      placeholder="ghes.company.com"
                      value={githubDomain}
                      onChange={(e) => setGithubDomain(e.target.value)}
                      disabled={copilotStatus === "polling"}
                    />
                  </Field>

                  {/* Device code actions */}
                  {copilotStatus === "idle" && (
                    <Button size="sm" className="w-full text-xs" onClick={handleCopilotLink}>
                      开始关联 GitHub 账户
                    </Button>
                  )}

                  {copilotStatus === "requesting" && (
                    <div className="flex items-center justify-center gap-2 py-2">
                      <RefreshCw className="h-4 w-4 animate-spin text-muted-foreground" />
                      <span className="text-xs text-muted-foreground">正在请求 GitHub 授权码...</span>
                    </div>
                  )}

                  {copilotStatus === "polling" && (
                    <div className="space-y-2 bg-muted/40 border border-border p-3 rounded-lg">
                      <div className="flex items-center justify-between">
                        <span className="text-[11px] text-muted-foreground font-semibold">GitHub 激活码</span>
                        <div className="flex gap-1.5">
                          <Button size="xs" variant="outline" className="h-6 text-[10px] gap-1 border-border" onClick={handleCopyCode}>
                            <Copy className="h-3 w-3" /> 复制
                          </Button>
                          <a
                            href={copilotVerifyUri}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="inline-flex items-center justify-center rounded-lg border border-border bg-background hover:bg-muted text-[10px] font-medium h-6 px-2 hover:text-foreground gap-1 select-none"
                          >
                            <ExternalLink className="h-3 w-3" /> 去浏览器激活
                          </a>
                        </div>
                      </div>
                      
                      <div className="text-center py-2.5 bg-card/65 border border-border rounded-md font-mono text-lg font-bold tracking-widest select-all text-foreground">
                        {copilotUserCode}
                      </div>

                      <div className="flex items-center justify-center gap-2 text-[10px] text-muted-foreground pt-1">
                        <RefreshCw className="h-3 w-3 animate-spin" />
                        <span>等待 GitHub 授权状态...</span>
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
                    <Field label="业务区域">
                      <Select value={zhipuRegion} onValueChange={(val) => { if (val) setZhipuRegion(val); }}>
                        <SelectTrigger
                          aria-label="业务区域"
                          className="w-full h-8 text-xs bg-card border-border text-foreground"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent className="bg-card border-border text-foreground">
                          <SelectItem value="cn">中国站 (open.bigmodel.cn)</SelectItem>
                          <SelectItem value="global">国际站 (api.z.ai)</SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  )}

                  {/* Zhipu Team Organization and Project IDs */}
                  {providerType === "zhipu_team" && (
                    <>
                      <Field label="Organization ID" description="智谱企业端给定的 Organization ID。">
                        <Input
                          aria-label="Organization ID"
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={zhipuTeamOrgId}
                          onChange={(e) => setZhipuTeamOrgId(e.target.value)}
                        />
                      </Field>
                      <Field label="Project ID" description="智谱关联项目的 Project ID。">
                        <Input
                          aria-label="Project ID"
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={zhipuTeamProjId}
                          onChange={(e) => setZhipuTeamProjId(e.target.value)}
                        />
                      </Field>
                    </>
                  )}

                  {/* MiniMax Region */}
                  {providerType === "minimax" && (
                    <Field label="平台接口域名">
                      <Select value={minimaxRegion} onValueChange={(val) => { if (val) setMinimaxRegion(val); }}>
                        <SelectTrigger
                          aria-label="平台接口域名"
                          className="w-full h-8 text-xs bg-card border-border text-foreground"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent className="bg-card border-border text-foreground">
                          <SelectItem value="cn">中国站 (api.minimaxi.com)</SelectItem>
                          <SelectItem value="global">全球站 (api.minimax.io)</SelectItem>
                        </SelectContent>
                      </Select>
                    </Field>
                  )}

                  {/* ZenMux URL */}
                  {providerType === "zenmux" && (
                    <Field label="额度查询 API 链接" description="ZenMux 提供的完整 HTTPS 额度状态查询接口。">
                      <Input
                        aria-label="额度查询 API 链接"
                        className="h-8 text-xs bg-card border-border text-foreground"
                        placeholder="https://zenmux.com/api/quota"
                        value={zenmuxUrl}
                        onChange={(e) => setZenmuxUrl(e.target.value)}
                      />
                    </Field>
                  )}

                  {/* Volcengine Region */}
                  {providerType === "volcengine" && (
                    <Field label="OpenAPI 默认服务区" description="火山控制台的默认 Region (一般为 cn-beijing)。">
                      <Input
                        aria-label="OpenAPI 默认服务区"
                        className="h-8 text-xs bg-card border-border text-foreground"
                        placeholder="cn-beijing"
                        value={volcengineRegion}
                        onChange={(e) => setVolcengineRegion(e.target.value)}
                      />
                    </Field>
                  )}

                  {/* Volcengine AK / SK */}
                  {providerType === "volcengine" ? (
                    <div className="space-y-3">
                      <Field label="AccessKey ID" description={editingAccount ? "留空表示不更新已存值" : undefined}>
                        <Input
                          aria-label="AccessKey ID"
                          className="h-8 text-xs bg-card border-border text-foreground"
                          value={accessKeyId}
                          onChange={(e) => setAccessKeyId(e.target.value)}
                        />
                      </Field>
                      <Field
                        label="SecretAccessKey"
                        description={editingAccount ? "留空表示不更新已存值" : undefined}
                      >
                        <div className="relative">
                          <Input
                            aria-label="SecretAccessKey"
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
                            aria-label={showSecret ? "隐藏 SecretAccessKey" : "显示 SecretAccessKey"}
                          >
                            {showSecret ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
                          </Button>
                        </div>
                      </Field>
                    </div>
                  ) : (
                    /* General API Key */
                    <Field label="API 密钥" description={editingAccount ? "留空表示不更新已保存密钥" : undefined}>
                      <div className="relative">
                        <Input
                          aria-label="API 密钥"
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
                          aria-label={showSecret ? "隐藏 API 密钥" : "显示 API 密钥"}
                        >
                          {showSecret ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
                        </Button>
                      </div>
                    </Field>
                  )}
                </>
              )}

              {/* Enabled Switch */}
              <div className="flex items-center justify-between border-t border-border/40 pt-3 text-xs mt-1">
                <div className="space-y-0.5">
                  <span className="font-semibold text-foreground">启用此账户监测</span>
                  <p className="text-[10px] text-muted-foreground">如果关闭，后台调度将暂停查询且不在主页中展示。</p>
                </div>
                <Switch
                  checked={enabled}
                  onCheckedChange={setEnabled}
                  aria-label="启用此账户监测"
                />
              </div>
            </div>

            <DialogFooter className="border-t border-border/40 pt-3 gap-2">
              <Button size="sm" variant="outline" className="text-xs h-8 border-border" onClick={() => { resetForm(); setIsFormOpen(false); }}>
                取消
              </Button>
              {providerType !== "copilot" && (
                <Button size="sm" className="text-xs h-8" onClick={handleFormSubmit} disabled={saveMutation.isPending}>
                  确定
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
                删除提供商确认
              </AlertDialogTitle>
              <AlertDialogDescription className="text-xs text-muted-foreground">
                你确定要删除 {deletingAccount.displayName} 吗？
                此操作将从软件中移除该账户配置及相关用量历史（不会影响系统的真实环境变量），删除后无法撤销。
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter className="gap-2">
              <AlertDialogCancel className="text-xs h-8 border-border bg-transparent hover:bg-muted">
                取消
              </AlertDialogCancel>
              <AlertDialogAction
                className="text-xs h-8 bg-status-danger text-white hover:bg-status-danger/90"
                onClick={() => deleteMutation.mutate(deletingAccount.id)}
              >
                确定删除
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}
