import * as React from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Info, Shield, KeyRound, Trash2, Globe } from "lucide-react";

import {
  getSettings,
  updateSettings,
  clearHistory,
} from "@/lib/backend";
import { getErrorMessage } from "@/lib/errors";
import {
  setLanguage,
  SUPPORTED_LANGUAGES,
  type SupportedLanguage,
} from "@/i18n";
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

const APP_VERSION = "0.2.1";
const REFRESH_OPTIONS = [5, 10, 15, 30] as const;

export function SettingsTab() {
  const { t, i18n } = useTranslation();
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
      toast.success(t("settings.toasts.updated"));
    },
    onError: (err) => {
      toast.error(t("settings.toasts.updateFailed", { message: getErrorMessage(err) }));
    },
  });

  // 3. Clear History Mutation
  const clearHistoryMutation = useMutation({
    mutationFn: clearHistory,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      setIsClearOpen(false);
      toast.success(t("settings.toasts.cleared"));
    },
    onError: (err) => {
      toast.error(t("settings.toasts.clearFailed", { message: getErrorMessage(err) }));
    },
  });

  const handleIntervalChange = (val: string | null) => {
    if (!settings || !val) return;
    const mins = parseInt(val, 10);
    if (!REFRESH_OPTIONS.includes(mins as (typeof REFRESH_OPTIONS)[number])) {
      toast.error(t("settings.behavior.invalidInterval"));
      return;
    }
    updateSettingsMutation.mutate({
      refreshIntervalMinutes: mins,
      accountOrder: settings.accountOrder ?? [],
    });
  };

  const handleLanguageChange = (val: string | null) => {
    if (!val) return;
    if ((SUPPORTED_LANGUAGES as readonly string[]).includes(val)) {
      setLanguage(val as SupportedLanguage);
    }
  };

  return (
    <div className="space-y-4">
      {/* ── Behavior Settings ─────────────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
            {t("settings.behavior.title")}
          </CardTitle>
          <CardDescription className="text-[10px] text-muted-foreground">
            {t("settings.behavior.desc")}
          </CardDescription>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3.5">
          <Field
            label={t("settings.language.label")}
            description={t("settings.language.desc")}
          >
            <Select
              value={i18n.language}
              onValueChange={handleLanguageChange}
            >
              <SelectTrigger
                aria-label={t("settings.language.label")}
                className="w-full h-8 text-xs bg-card border-border text-foreground"
              >
                <Globe className="h-3.5 w-3.5 mr-1.5 text-muted-foreground" />
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="bg-card border-border text-foreground">
                {SUPPORTED_LANGUAGES.map((lang) => (
                  <SelectItem key={lang} value={lang}>
                    {t(`settings.language.options.${lang}`)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>

          <Field
            label={t("settings.behavior.refreshLabel")}
            description={t("settings.behavior.refreshDesc")}
          >
            <Select
              value={(settings?.refreshIntervalMinutes ?? 5).toString()}
              onValueChange={handleIntervalChange}
              disabled={updateSettingsMutation.isPending}
            >
              <SelectTrigger
                aria-label={t("settings.behavior.refreshLabel")}
                className="w-full h-8 text-xs bg-card border-border text-foreground"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="bg-card border-border text-foreground">
                {REFRESH_OPTIONS.map((mins) => (
                  <SelectItem key={mins} value={mins.toString()}>
                    {t(`settings.behavior.options.${mins}`)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        </CardContent>
      </Card>

      {/* ── Data & History Management ────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
            {t("settings.data.title")}
          </CardTitle>
          <CardDescription className="text-[10px] text-muted-foreground">
            {t("settings.data.desc")}
          </CardDescription>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3">
          <div className="flex items-start gap-2 text-xs">
            <Info className="h-4 w-4 text-muted-foreground mt-0.5" />
            <div className="space-y-0.5 flex-1">
              <span className="font-semibold text-foreground">{t("settings.data.autoCleanup")}</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                {t("settings.data.autoCleanupDesc")}
              </p>
            </div>
          </div>

          <div className="flex items-center justify-between border-t border-border/40 pt-3 text-xs mt-1">
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">{t("settings.data.clearHistory")}</span>
              <p className="text-[10px] text-muted-foreground">{t("settings.data.clearHistoryDesc")}</p>
            </div>
            <Button
              variant="outline"
              size="sm"
              className="text-xs h-8 border-status-danger text-status-danger hover:bg-status-danger/10 hover:text-status-danger"
              onClick={() => setIsClearOpen(true)}
            >
              <Trash2 className="h-3.5 w-3.5 mr-1" /> {t("settings.data.clearButton")}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* ── Security Disclosures ─────────────────────────────────── */}
      <Card className="border-border bg-card/20 shadow-sm">
        <CardHeader className="p-4 pb-2">
          <CardTitle className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
            {t("settings.security.title")}
          </CardTitle>
        </CardHeader>
        <CardContent className="p-4 pt-1 space-y-3 text-xs">
          <div className="flex items-start gap-2.5">
            <Shield className="h-4 w-4 text-status-safe mt-0.5" />
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">{t("settings.security.envRead")}</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                {t("settings.security.envReadDesc")}
              </p>
            </div>
          </div>

          <div className="flex items-start gap-2.5">
            <KeyRound className="h-4 w-4 text-muted-foreground mt-0.5" />
            <div className="space-y-0.5">
              <span className="font-semibold text-foreground">{t("settings.security.noRelay")}</span>
              <p className="text-[10px] text-muted-foreground leading-normal">
                {t("settings.security.noRelayDesc")}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* ── App Version ───────────────────────────────────────────── */}
      <div className="text-center text-[10px] text-muted-foreground py-2 tracking-wider">
        {t("settings.version", { version: APP_VERSION })}
      </div>

      {/* ── Clear History Confirmation ────────────────────────────── */}
      {isClearOpen && (
        <AlertDialog open={isClearOpen} onOpenChange={setIsClearOpen}>
          <AlertDialogContent className="bg-card border-border text-foreground">
            <AlertDialogHeader>
              <AlertDialogTitle className="text-sm font-bold text-status-danger flex items-center gap-1.5">
                <Trash2 className="h-4 w-4" /> {t("settings.data.dialogTitle")}
              </AlertDialogTitle>
              <AlertDialogDescription className="text-xs text-muted-foreground">
                {t("settings.data.dialogDesc")}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter className="gap-2">
              <AlertDialogCancel className="text-xs h-8 border-border bg-transparent hover:bg-muted">
                {t("settings.data.cancel")}
              </AlertDialogCancel>
              <AlertDialogAction
                className="text-xs h-8 bg-status-danger text-white hover:bg-status-danger/90"
                onClick={() => clearHistoryMutation.mutate()}
                disabled={clearHistoryMutation.isPending}
              >
                {t("settings.data.confirm")}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}
    </div>
  );
}
