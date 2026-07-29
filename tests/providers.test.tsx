import * as React from "react";
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ProvidersTab } from "@/components/ProvidersTab";
import type {
  AccountDashboard,
  DashboardSnapshot,
  PublicAccount,
  TierDashboard,
} from "@/types";

const toastError = vi.fn();
const toastSuccess = vi.fn();
const toastInfo = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
    success: (...args: unknown[]) => toastSuccess(...args),
    info: (...args: unknown[]) => toastInfo(...args),
  },
}));

vi.mock("@/lib/backend", () => ({
  listAccounts: vi.fn(),
  saveAccount: vi.fn(),
  deleteAccount: vi.fn(),
  discoverEnvAccounts: vi.fn(),
  probeCliCredentials: vi.fn(),
  startCopilotDeviceFlow: vi.fn(),
  pollCopilotDeviceFlow: vi.fn(),
  getSettings: vi.fn(),
  getDashboard: vi.fn(),
  requestNotificationPermission: vi.fn(),
}));

import {
  listAccounts,
  saveAccount,
  probeCliCredentials,
  getSettings,
  getDashboard,
  requestNotificationPermission,
  discoverEnvAccounts,
} from "@/lib/backend";

const NOW = Date.now();

const kimiAccount: PublicAccount = {
  id: "kimi:1",
  provider: "kimi",
  displayName: "Kimi Personal",
  enabled: true,
  credentialSource: "env",
  hasCredential: true,
  config: { type: "kimi" },
  alertRules: [],
};

const claudeCliAccount: PublicAccount = {
  id: "cli:claude",
  provider: "claude",
  displayName: "Claude (CLI)",
  enabled: true,
  credentialSource: "cli_auto",
  hasCredential: true,
  config: { type: "claude" },
  alertRules: [
    { tierId: "five_hour", enabled: true, thresholdPercent: 70 },
    { tierId: "seven_day", enabled: false, thresholdPercent: 80 },
  ],
};

const fiveHour: TierDashboard = {
  quota: {
    id: "five_hour",
    label: "5-Hour Session",
    utilization: 40,
    resetsAt: NOW + 3 * 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 1,
    projectedUtilizationAtReset: 50,
    exhaustionAt: null,
    sampleCount: 5,
    observationMinutes: 30,
  },
};

const sevenDay: TierDashboard = {
  quota: {
    id: "seven_day",
    label: "7-Day Limit",
    utilization: 20,
    resetsAt: NOW + 4 * 24 * 60 * 60 * 1000,
    unlimited: false,
  },
  forecast: {
    state: "safe",
    ratePerHour: 0.2,
    projectedUtilizationAtReset: 25,
    exhaustionAt: null,
    sampleCount: 8,
    observationMinutes: 120,
  },
};

const unlimitedTier: TierDashboard = {
  quota: {
    id: "unlimited_pool",
    label: "Unlimited Pool",
    utilization: 0,
    resetsAt: null,
    unlimited: true,
  },
  forecast: {
    state: "safe",
    ratePerHour: 0,
    projectedUtilizationAtReset: 0,
    exhaustionAt: null,
    sampleCount: 0,
    observationMinutes: 0,
  },
};

function dashFor(account: PublicAccount, tiers: TierDashboard[]): DashboardSnapshot {
  const row: AccountDashboard = {
    account,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers,
  };
  return {
    accounts: [row],
    leadingRisk: "safe",
    refreshedAt: NOW,
    nextRefreshAt: NOW + 60_000,
    refreshInProgress: false,
  };
}

function renderTab() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <ProvidersTab />
    </QueryClientProvider>
  );
}

describe("ProvidersTab alert rules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    toastError.mockReset();
    vi.mocked(getSettings).mockResolvedValue({
      refreshIntervalMinutes: 5,
      accountOrder: [],
    });
    vi.mocked(discoverEnvAccounts).mockResolvedValue([]);
    vi.mocked(probeCliCredentials).mockResolvedValue([
      {
        provider: "claude",
        status: "valid",
        message: null,
        accountId: "cli:claude",
      },
      { provider: "codex", status: "not_found", message: null, accountId: "cli:codex" },
      { provider: "gemini", status: "not_found", message: null, accountId: "cli:gemini" },
    ]);
    vi.mocked(requestNotificationPermission).mockResolvedValue(true);
    vi.mocked(saveAccount).mockImplementation(async (input) => ({
      id: input.id ?? "new",
      provider: input.config.type as PublicAccount["provider"],
      displayName: input.displayName,
      enabled: input.enabled,
      credentialSource: "env",
      hasCredential: true,
      config: input.config,
      alertRules: input.alertRules ?? [],
    }));
  });

  it("defaults new finite tiers to off + 80 and allows independent multi-tier edits", async () => {
    vi.mocked(listAccounts).mockResolvedValue([kimiAccount]);
    vi.mocked(getDashboard).mockResolvedValue(
      dashFor(kimiAccount, [fiveHour, sevenDay, unlimitedTier])
    );

    renderTab();
    await screen.findByText("Kimi Personal");
    fireEvent.click(screen.getByLabelText("编辑 Kimi Personal"));

    expect(await screen.findByText("额度告警")).toBeInTheDocument();
    expect(screen.getByText("无限额度无需告警")).toBeInTheDocument();

    const fiveSwitch = screen.getByLabelText("启用 5-Hour Session 告警");
    const sevenSwitch = screen.getByLabelText("启用 7-Day Limit 告警");
    expect(fiveSwitch).not.toBeChecked();
    expect(sevenSwitch).not.toBeChecked();
    expect(screen.getByLabelText("5-Hour Session 告警阈值")).toHaveValue(80);
    expect(screen.getByLabelText("7-Day Limit 告警阈值")).toHaveValue(80);

    fireEvent.click(fiveSwitch);
    await waitFor(() => expect(requestNotificationPermission).toHaveBeenCalled());
    fireEvent.change(screen.getByLabelText("5-Hour Session 告警阈值"), {
      target: { value: "60" },
    });
    fireEvent.change(screen.getByLabelText("7-Day Limit 告警阈值"), {
      target: { value: "90" },
    });

    fireEvent.click(screen.getByRole("button", { name: "确定" }));
    await waitFor(() => expect(saveAccount).toHaveBeenCalled());
    const payload = vi.mocked(saveAccount).mock.calls[0][0];
    expect(payload.alertRules).toEqual(
      expect.arrayContaining([
        { tierId: "five_hour", enabled: true, thresholdPercent: 60 },
        { tierId: "seven_day", enabled: false, thresholdPercent: 90 },
        { tierId: "unlimited_pool", enabled: false, thresholdPercent: 80 },
      ])
    );
  });

  it("accepts 1/99 and rejects 0/100/decimal thresholds", async () => {
    vi.mocked(listAccounts).mockResolvedValue([kimiAccount]);
    vi.mocked(getDashboard).mockResolvedValue(dashFor(kimiAccount, [fiveHour]));

    renderTab();
    await screen.findByText("Kimi Personal");
    fireEvent.click(screen.getByLabelText("编辑 Kimi Personal"));
    const input = await screen.findByLabelText("5-Hour Session 告警阈值");
    const save = screen.getByRole("button", { name: "确定" });

    fireEvent.change(input, { target: { value: "1" } });
    expect(save).not.toBeDisabled();
    fireEvent.change(input, { target: { value: "99" } });
    expect(save).not.toBeDisabled();

    fireEvent.change(input, { target: { value: "0" } });
    expect(save).toBeDisabled();
    fireEvent.change(input, { target: { value: "100" } });
    expect(save).toBeDisabled();
    fireEvent.change(input, { target: { value: "3.5" } });
    expect(save).toBeDisabled();
    expect(screen.getByText("请输入 1–99 的整数")).toBeInTheDocument();
  });

  it("rolls back switch when notification permission is denied or errors", async () => {
    vi.mocked(listAccounts).mockResolvedValue([kimiAccount]);
    vi.mocked(getDashboard).mockResolvedValue(dashFor(kimiAccount, [fiveHour]));
    vi.mocked(requestNotificationPermission).mockResolvedValueOnce(false);

    renderTab();
    await screen.findByText("Kimi Personal");
    fireEvent.click(screen.getByLabelText("编辑 Kimi Personal"));
    const sw = await screen.findByLabelText("启用 5-Hour Session 告警");
    fireEvent.click(sw);
    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith("系统通知权限未授予，无法开启额度告警")
    );
    expect(sw).not.toBeChecked();

    vi.mocked(requestNotificationPermission).mockRejectedValueOnce(new Error("boom"));
    fireEvent.click(sw);
    await waitFor(() => expect(toastError).toHaveBeenCalledTimes(2));
    expect(sw).not.toBeChecked();
  });

  it("opens CLI alert settings as a read-only account form and refills rules", async () => {
    vi.mocked(listAccounts).mockResolvedValue([claudeCliAccount, kimiAccount]);
    vi.mocked(getDashboard).mockResolvedValue(
      dashFor(claudeCliAccount, [fiveHour, sevenDay])
    );

    renderTab();
    const alertBtn = await screen.findByRole("button", { name: "告警设置" });
    fireEvent.click(alertBtn);

    expect(await screen.findByText("告警设置: Claude (CLI)")).toBeInTheDocument();
    expect(screen.queryByLabelText("提供商类型")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("启用此账户监测")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("显示名称")).not.toBeInTheDocument();
    expect(screen.getByLabelText("启用 5-Hour Session 告警")).toBeChecked();
    expect(screen.getByLabelText("5-Hour Session 告警阈值")).toHaveValue(70);
    expect(screen.getByLabelText("启用 7-Day Limit 告警")).not.toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "确定" }));
    await waitFor(() => expect(saveAccount).toHaveBeenCalled());
    const payload = vi.mocked(saveAccount).mock.calls[0][0];
    expect(payload.id).toBe("cli:claude");
    expect(payload.displayName).toBe("Claude (CLI)");
    expect(payload.enabled).toBe(true);
    expect(payload.config).toEqual({ type: "claude" });
    expect(payload.alertRules).toEqual(
      expect.arrayContaining([
        { tierId: "five_hour", enabled: true, thresholdPercent: 70 },
        { tierId: "seven_day", enabled: false, thresholdPercent: 80 },
      ])
    );
  });
});
