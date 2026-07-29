import * as React from "react";
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@/components/ui/tooltip";
import { OverviewTab } from "@/components/OverviewTab";
import type {
  AccountDashboard,
  DashboardSnapshot,
  HistoryPoint,
  PublicAccount,
  TierDashboard,
} from "@/types";

vi.mock("@/lib/backend", () => ({
  getDashboard: vi.fn(),
  refreshAll: vi.fn(),
  getTierHistory: vi.fn(),
  openMainWindow: vi.fn(),
  getSettings: vi.fn(async () => ({
    refreshIntervalMinutes: 5,
    accountOrder: [],
  })),
  updateAccountOrder: vi.fn(async () => undefined),
}));

import { getDashboard, refreshAll, getTierHistory, getSettings, updateAccountOrder } from "@/lib/backend";

const NOW = Date.now();

const geminiProAccount: PublicAccount = {
  id: "safe-acc",
  provider: "gemini",
  displayName: "Gemini Pro",
  enabled: true,
  credentialSource: "env",
  hasCredential: true,
  config: { type: "gemini" },
  alertRules: [],
};

const claudeAccount: PublicAccount = {
  id: "at-risk-acc",
  provider: "claude",
  displayName: "Claude",
  enabled: true,
  credentialSource: "cli_auto",
  hasCredential: true,
  config: { type: "claude" },
  alertRules: [],
};

const volcengineAccount: PublicAccount = {
  id: "cred-error-acc",
  provider: "volcengine",
  displayName: "Volcengine Ark",
  enabled: true,
  credentialSource: "env",
  hasCredential: false,
  config: { type: "volcengine", region: "cn-beijing" },
  alertRules: [],
};

const copilotAccount: PublicAccount = {
  id: "copilot-acc",
  provider: "copilot",
  displayName: "Copilot Business",
  enabled: true,
  credentialSource: "env",
  hasCredential: true,
  config: { type: "copilot", githubDomain: null },
  alertRules: [],
};

const fiveHourClaude: TierDashboard = {
  quota: {
    id: "five_hour",
    label: "5-Hour Session",
    utilization: 92.5,
    resetsAt: NOW + 3 * 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "at_risk",
    ratePerHour: 12.0,
    projectedUtilizationAtReset: 100.0,
    exhaustionAt: NOW + 45 * 60 * 1000,
    sampleCount: 15,
    observationMinutes: 90,
  },
};

const sevenDayClaude: TierDashboard = {
  quota: {
    id: "seven_day",
    label: "7-Day Limit",
    utilization: 34.0,
    resetsAt: NOW + 4 * 24 * 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 0.8,
    projectedUtilizationAtReset: 41.2,
    exhaustionAt: null,
    sampleCount: 22,
    observationMinutes: 300,
  },
};

const monthlyClaude: TierDashboard = {
  quota: {
    id: "monthly",
    label: "Monthly Limit",
    utilization: 12.4,
    resetsAt: NOW + 21 * 24 * 60 * 60 * 1000,
    used: 620,
    limit: 5000,
    unit: "credits",
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 0.1,
    projectedUtilizationAtReset: 18.0,
    exhaustionAt: null,
    sampleCount: 18,
    observationMinutes: 480,
  },
};

const geminiProTier: TierDashboard = {
  quota: {
    id: "gemini_pro",
    label: "Gemini Pro",
    utilization: 15.0,
    resetsAt: NOW + 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 2.0,
    projectedUtilizationAtReset: 17.0,
    exhaustionAt: null,
    sampleCount: 10,
    observationMinutes: 60,
  },
};

const geminiFlashTier: TierDashboard = {
  quota: {
    id: "gemini_flash",
    label: "Gemini Flash",
    utilization: 63.0,
    resetsAt: NOW + 9 * 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 1.1,
    projectedUtilizationAtReset: 72.0,
    exhaustionAt: null,
    sampleCount: 9,
    observationMinutes: 120,
  },
};

const copilotMonthlyUnlimited: TierDashboard = {
  quota: {
    id: "monthly",
    label: "Monthly Limit",
    utilization: 0.0,
    resetsAt: null,
    used: null,
    limit: null,
    unit: null,
    unlimited: true,
  },
  forecast: {
    state: "safe",
    ratePerHour: 0.0,
    projectedUtilizationAtReset: 0.0,
    exhaustionAt: null,
    sampleCount: 0,
    observationMinutes: 0,
  },
};

const accounts: AccountDashboard[] = [
  {
    account: geminiProAccount,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers: [geminiProTier, geminiFlashTier],
  },
  {
    account: claudeAccount,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers: [fiveHourClaude, sevenDayClaude, monthlyClaude],
  },
  {
    account: volcengineAccount,
    credentialStatus: "expired",
    stale: false,
    error: "AccessKey ID / Secret are incorrect.",
    tiers: [],
  },
  {
    account: copilotAccount,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers: [copilotMonthlyUnlimited],
  },
];

const mockDashboard: DashboardSnapshot = {
  accounts,
  leadingRisk: "at_risk",
  refreshedAt: NOW - 30000,
  nextRefreshAt: NOW + 5 * 60 * 1000,
  refreshInProgress: false,
};

const mockHistory: HistoryPoint[] = [
  { sampledAt: NOW - 60 * 60 * 1000, utilization: 20.0, resetsAt: null },
  { sampledAt: NOW - 30 * 60 * 1000, utilization: 50.0, resetsAt: null },
  { sampledAt: NOW, utilization: 92.5, resetsAt: null },
];

function renderWithProviders(ui: React.ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>{ui}</TooltipProvider>
    </QueryClientProvider>,
  );
}

describe("OverviewTab", () => {
  beforeEach(() => {
    vi.mocked(getSettings).mockResolvedValue({
      refreshIntervalMinutes: 5,
      accountOrder: [],
    });
    vi.mocked(updateAccountOrder).mockClear();
  });

  it("renders dashboard with RiskHero and sorted accounts", async () => {
    vi.mocked(getDashboard).mockResolvedValue(mockDashboard);
    vi.mocked(refreshAll).mockResolvedValue(mockDashboard);
    vi.mocked(getTierHistory).mockResolvedValue(mockHistory);

    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={() => {}}
        onNavigateToSettings={() => {}}
      />,
    );

    // 1. RiskHero: at-risk first
    const hero = await screen.findAllByText("Claude");
    expect(hero.length).toBeGreaterThan(0);
    expect(await screen.findAllByText(/有耗尽风险|领先重置/)).toBeTruthy();

    // 2. Account cards rendered
    expect(await screen.findAllByText("Gemini Pro")).toBeTruthy();
    expect(await screen.findAllByText("Volcengine Ark")).toBeTruthy();
    expect(await screen.findAllByText("Copilot Business")).toBeTruthy();

    // 3. Sorting priority: Claude (at_risk) should appear before Gemini Pro (safe)
    const allCards = screen.getAllByText(/.+/);
    const claudeIdx = allCards.findIndex((el) => el.textContent === "Claude");
    const geminiIdx = allCards.findIndex((el) => el.textContent === "Gemini Pro");
    expect(claudeIdx).toBeLessThan(geminiIdx);
  });

  it("respects persisted manual account order and can reset to risk sort", async () => {
    vi.mocked(getDashboard).mockResolvedValue(mockDashboard);
    vi.mocked(refreshAll).mockResolvedValue(mockDashboard);
    vi.mocked(getTierHistory).mockResolvedValue(mockHistory);
    vi.mocked(getSettings).mockResolvedValue({
      refreshIntervalMinutes: 5,
      // Put safe Gemini ahead of at-risk Claude
      accountOrder: ["safe-acc", "at-risk-acc", "cred-error-acc", "copilot-acc"],
    });

    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={() => {}}
        onNavigateToSettings={() => {}}
      />,
    );

    const geminiCard = await screen.findByRole("button", {
      name: "拖动排序 Gemini Pro",
    });
    const claudeCard = screen.getByRole("button", { name: "拖动排序 Claude" });
    expect(
      geminiCard.compareDocumentPosition(claudeCard) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    const reset = await screen.findByRole("button", { name: "恢复风险排序" });
    expect(reset).toBeEnabled();
    reset.click();
    await waitFor(() => {
      expect(vi.mocked(updateAccountOrder)).toHaveBeenCalled();
    });
    expect(vi.mocked(updateAccountOrder).mock.calls.at(-1)?.[0]).toEqual([]);
  });

  it("shows every tier per account, including absolute and unlimited", async () => {
    vi.mocked(getDashboard).mockResolvedValue(mockDashboard);
    vi.mocked(refreshAll).mockResolvedValue(mockDashboard);
    vi.mocked(getTierHistory).mockResolvedValue(mockHistory);

    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={() => {}}
        onNavigateToSettings={() => {}}
      />,
    );

    // (a) Claude card shows all three tier labels simultaneously.
    expect(await screen.findByText("5-Hour Session")).toBeInTheDocument();
    expect(screen.getByText("7-Day Limit")).toBeInTheDocument();
    expect(screen.getAllByText("Monthly Limit").length).toBeGreaterThan(0);

    // (b) Absolute quota 620 / 5,000 credits visible in Claude card.
    expect(screen.getByText("620 / 5,000 credits")).toBeInTheDocument();

    // (c) Copilot card shows 无限额度 and does NOT show "0.0%".
    expect(screen.getAllByText("无限额度").length).toBeGreaterThan(0);
    const card = screen
      .getByText("Copilot Business")
      .closest("[data-account-id]") as HTMLElement | null;
    expect(card).toBeTruthy();
    expect(card?.textContent).not.toMatch(/0\.0%/);
  });

  it("queries history per (accountId, tierId) when switching tiers in the dialog", async () => {
    vi.mocked(getDashboard).mockResolvedValue(mockDashboard);
    vi.mocked(refreshAll).mockResolvedValue(mockDashboard);
    vi.mocked(getTierHistory).mockResolvedValue(mockHistory);

    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={() => {}}
        onNavigateToSettings={() => {}}
      />,
    );

    await waitFor(() => {
      expect(getDashboard).toHaveBeenCalled();
    });

    // Click the seven_day tier row of the Claude account.
    const weeklyRow = await screen.findByText("7-Day Limit");
    const weeklyButton = weeklyRow.closest(
      "button[data-tier-id='seven_day']",
    ) as HTMLButtonElement | null;
    expect(weeklyButton).toBeTruthy();
    fireEvent.click(weeklyButton!);

    // (d) Dialog title shows Claude · 7-Day Limit and getTierHistory called with seven_day.
    expect(await screen.findByText("Claude · 7-Day Limit")).toBeInTheDocument();
    await waitFor(() => {
      expect(getTierHistory).toHaveBeenLastCalledWith("at-risk-acc", "seven_day", 24);
    });

    // (e) Switching to the monthly tier button updates the queried tier id.
    const monthlySwitch = (await screen.findAllByText("Monthly Limit")).find((el) =>
      el.closest("button[data-tier-switch-id='monthly']"),
    ) as HTMLElement | undefined;
    expect(monthlySwitch).toBeTruthy();
    fireEvent.click(monthlySwitch!);
    await waitFor(() => {
      expect(getTierHistory).toHaveBeenLastCalledWith("at-risk-acc", "monthly", 24);
    });
  });

  it("renders empty state when there are no accounts", async () => {
    vi.mocked(getDashboard).mockResolvedValue({
      ...mockDashboard,
      accounts: [],
    });
    vi.mocked(refreshAll).mockResolvedValue(mockDashboard);
    vi.mocked(getTierHistory).mockResolvedValue(mockHistory);

    const mockNav = vi.fn();
    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={mockNav}
        onNavigateToSettings={() => {}}
      />,
    );

    // Empty state shown
    expect(await screen.findByText(/暂无启用的提供商/)).toBeInTheDocument();
    expect(screen.getByText(/添加提供商/)).toBeInTheDocument();

    // Clicking "添加提供商" triggers navigation callback
    const addBtn = screen.getByText(/添加提供商/);
    fireEvent.click(addBtn);
    expect(mockNav).toHaveBeenCalled();
  });

  it("renders dashboard failures with retry instead of the empty state", async () => {
    vi.mocked(getDashboard).mockRejectedValue(new Error("IPC unavailable"));
    vi.mocked(refreshAll).mockRejectedValue(new Error("IPC unavailable"));

    renderWithProviders(
      <OverviewTab
        onNavigateToProviders={() => {}}
        onNavigateToSettings={() => {}}
      />,
    );

    expect(await screen.findByText("IPC unavailable")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
    expect(screen.queryByText(/暂无启用的提供商/)).not.toBeInTheDocument();
  });
});
