import * as React from "react";
import { useTranslation } from "react-i18next";
import "./i18n";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { OverviewTab } from "@/components/OverviewTab";
import { ProvidersTab } from "@/components/ProvidersTab";
import { SettingsTab } from "@/components/SettingsTab";
import { TrayPanel } from "@/components/TrayPanel";
import { BarChart3, Database, Settings } from "lucide-react";
import { useDashboardSnapshot } from "@/hooks/useDashboardSnapshot";
import { useWidgetSync } from "@/hooks/useWidgetSync";

const isTraySurface =
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("surface") === "tray";

function App() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = React.useState("overview");
  const { query: dashboardQuery } = useDashboardSnapshot();

  // Push the latest dashboard snapshot to the native widget extension.
  // The hook throttles internally (60s) and no-ops outside Tauri.
  useWidgetSync(dashboardQuery.data);

  if (isTraySurface) {
    return <TrayPanel />;
  }

  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col font-sans select-none antialiased">
      {/* ── App Title Bar ── */}
      <header className="px-4 py-3 border-b border-border/60 bg-card/25 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <img src="/last-token.svg" alt={t("app.logoAlt")} className="h-5 w-5 rounded object-contain" />
          <h1 className="text-sm font-bold tracking-tight text-foreground">{t("app.name")}</h1>
        </div>
        <div className="flex items-center gap-1">
          <span className="h-1.5 w-1.5 rounded-full bg-status-safe animate-pulse" />
          <span className="text-[10px] text-muted-foreground font-semibold uppercase tracking-wider">
            {t("app.running")}
          </span>
        </div>
      </header>

      {/* ── Main Tab Views ── */}
      <main className="flex-1 p-4 overflow-y-auto pb-20">
        <Tabs value={activeTab} onValueChange={setActiveTab} className="w-full space-y-4">
          <TabsList className="grid grid-cols-3 w-full h-9 bg-card border border-border/80 p-0.5 rounded-lg">
            <TabsTrigger
              value="overview"
              className="text-xs py-1 flex items-center justify-center gap-1.5 rounded-md data-[state=active]:bg-background data-[state=active]:text-foreground text-muted-foreground transition-all duration-200"
            >
              <BarChart3 className="h-3.5 w-3.5" />
              {t("tabs.overview")}
            </TabsTrigger>
            <TabsTrigger
              value="providers"
              className="text-xs py-1 flex items-center justify-center gap-1.5 rounded-md data-[state=active]:bg-background data-[state=active]:text-foreground text-muted-foreground transition-all duration-200"
            >
              <Database className="h-3.5 w-3.5" />
              {t("tabs.providers")}
            </TabsTrigger>
            <TabsTrigger
              value="settings"
              className="text-xs py-1 flex items-center justify-center gap-1.5 rounded-md data-[state=active]:bg-background data-[state=active]:text-foreground text-muted-foreground transition-all duration-200"
            >
              <Settings className="h-3.5 w-3.5" />
              {t("tabs.settings")}
            </TabsTrigger>
          </TabsList>

          <TabsContent value="overview" className="mt-0 focus-visible:outline-none">
            <OverviewTab
              onNavigateToSettings={() => setActiveTab("settings")}
              onNavigateToProviders={() => setActiveTab("providers")}
            />
          </TabsContent>

          <TabsContent value="providers" className="mt-0 focus-visible:outline-none">
            <ProvidersTab />
          </TabsContent>

          <TabsContent value="settings" className="mt-0 focus-visible:outline-none">
            <SettingsTab />
          </TabsContent>
        </Tabs>
      </main>
    </div>
  );
}

export default App;
