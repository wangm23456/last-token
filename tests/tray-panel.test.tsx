import * as React from "react";
import { afterEach, beforeEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@/components/ui/tooltip";
import { TrayPanel } from "@/components/TrayPanel";
import type {
  AccountDashboard,
  DashboardSnapshot,
  PublicAccount,
  TierDashboard,
} from "@/types";

vi.mock("@/lib/backend", () => ({
  getDashboard: vi.fn(),
  refreshAll: vi.fn(),
  getTierHistory: vi.fn(),
  openMainWindow: vi.fn(),
}));

// The hook dynamically imports @tauri-apps/api/event inside an effect.
// We hoist the listen spy so the effect picks up the same vi.fn().
const listenSpy = vi.fn<[], Promise<() => void>>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (..._args: unknown[]) => listenSpy(..._args),
}));

import { getDashboard, refreshAll, openMainWindow } from "@/lib/backend";

const NOW = Date.now();

const accounts: AccountDashboard[] = [
  {
    account: {
      id: "claude-1",
      provider: "claude",
      displayName: "Claude Pro",
      enabled: true,
      credentialSource: "env",
      hasCredential: true,
      config: { type: "claude" },
    } satisfies PublicAccount,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers: [
      {
        quota: {
          id: "five_hour",
          label: "5-Hour Session",
          utilization: 78.0,
          resetsAt: NOW + 2 * 60 * 60 * 1000,
          unlimited: false,
        },
        forecast: {
          state: "at_risk",
          ratePerHour: 8.0,
          projectedUtilizationAtReset: 99.0,
          exhaustionAt: NOW + 30 * 60 * 1000,
          sampleCount: 12,
          observationMinutes: 80,
        },
      },
      {
        quota: {
          id: "seven_day",
          label: "7-Day Limit",
          utilization: 32.0,
          resetsAt: NOW + 4 * 24 * 60 * 60 * 1000,
          unlimited: false,
        },
        forecast: {
          state: "safe",
          ratePerHour: 0.5,
          projectedUtilizationAtReset: 38.0,
          exhaustionAt: null,
          sampleCount: 18,
          observationMinutes: 240,
        },
      },
    ] satisfies TierDashboard[],
  },
  {
    account: {
      id: "copilot-1",
      provider: "copilot",
      displayName: "Copilot Business",
      enabled: true,
      credentialSource: "env",
      hasCredential: true,
      config: { type: "copilot", githubDomain: null },
    } satisfies PublicAccount,
    credentialStatus: "valid",
    stale: false,
    error: null,
    tiers: [
      {
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
      },
    ] satisfies TierDashboard[],
  },
  {
    account: {
      id: "volcengine-1",
      provider: "volcengine",
      displayName: "Volcengine Ark",
      enabled: true,
      credentialSource: "env",
      hasCredential: false,
      config: { type: "volcengine", region: "cn-beijing" },
    } satisfies PublicAccount,
    credentialStatus: "expired",
    stale: false,
    error: "AccessKey ID / Secret are incorrect.",
    tiers: [],
  },
];

const baseSnapshot: DashboardSnapshot = {
  accounts,
  leadingRisk: "at_risk",
  refreshedAt: NOW - 30 * 1000,
  nextRefreshAt: NOW + 5 * 60 * 1000,
  refreshInProgress: false,
};

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

describe("TrayPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenSpy.mockReset();
    listenSpy.mockResolvedValue(() => {});
    (globalThis as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    delete (globalThis as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it("renders every account with all tier labels, unlimited text, and footer actions", async () => {
    vi.mocked(getDashboard).mockResolvedValue(baseSnapshot);
    vi.mocked(refreshAll).mockResolvedValue(baseSnapshot);

    renderWithProviders(<TrayPanel />);

    // Multi-account: Claude and Copilot and Volcengine visible.
    expect(await screen.findByText("Claude Pro")).toBeInTheDocument();
    expect(screen.getByText("Copilot Business")).toBeInTheDocument();
    expect(screen.getByText("Volcengine Ark")).toBeInTheDocument();

    // The "5-Hour Session" label appears in both the per-account tier row
    // and the global risk line — use getAllByText to assert presence.
    expect(screen.getAllByText("5-Hour Session").length).toBeGreaterThan(0);
    expect(screen.getByText("7-Day Limit")).toBeInTheDocument();
    // Copilot monthly tier label is unique to the unlimited row.
    expect(screen.getByText("Monthly Limit")).toBeInTheDocument();

    // Copilot unlimited text appears.
    expect(screen.getAllByText("无限额度").length).toBeGreaterThan(0);

    // Volcengine error path.
    expect(screen.getByText(/AccessKey ID/)).toBeInTheDocument();

    // 打开主界面 button wired to openMainWindow.
    const openBtn = screen.getByRole("button", { name: /打开主界面/ });
    fireEvent.click(openBtn);
    expect(openMainWindow).toHaveBeenCalled();
  });

  it("disables the refresh buttons while the snapshot is refreshing", async () => {
    const refreshingSnapshot: DashboardSnapshot = {
      ...baseSnapshot,
      refreshInProgress: true,
    };
    vi.mocked(getDashboard).mockResolvedValue(refreshingSnapshot);
    vi.mocked(refreshAll).mockResolvedValue(refreshingSnapshot);

    renderWithProviders(<TrayPanel />);

    await screen.findByText("Claude Pro");

    const headerRefresh = screen.getByRole("button", { name: /刷新额度/ });
    const footerRefresh = screen.getByRole("button", { name: /立即刷新/ });
    expect(headerRefresh).toBeDisabled();
    expect(footerRefresh).toBeDisabled();
  });

  it("applies quota-updated event payload directly without re-fetching", async () => {
    vi.mocked(getDashboard).mockResolvedValue(baseSnapshot);
    vi.mocked(refreshAll).mockResolvedValue(baseSnapshot);

    renderWithProviders(<TrayPanel />);

    // Wait for first fetch and listen() to register.
    await waitFor(() => {
      expect(getDashboard).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(listenSpy).toHaveBeenCalled();
    });

    const listenCall = listenSpy.mock.calls[0]!;
    const eventName = listenCall[0] as string;
    const handler = listenCall[1] as (e: { payload: unknown }) => void;
    expect(eventName).toBe("quota-updated");

    // Build a modified payload: swap a tier label so we can detect cache update.
    const updatedSnapshot: DashboardSnapshot = {
      ...baseSnapshot,
      accounts: accounts.map((acc) =>
        acc.account.id === "claude-1"
          ? {
              ...acc,
              tiers: acc.tiers.map((t) =>
                t.quota.id === "five_hour"
                  ? {
                      ...t,
                      quota: { ...t.quota, label: "5-Hour Limit (Updated)" },
                    }
                  : t,
              ),
            }
          : acc,
      ),
    };

    handler({ payload: updatedSnapshot });

    // New label appears (both per-tier row AND global risk line update).
    expect((await screen.findAllByText("5-Hour Limit (Updated)")).length).toBeGreaterThan(0);
    expect(getDashboard).toHaveBeenCalledTimes(1);

    // Empty payload triggers invalidateQueries → refetch.
    handler({ payload: null });
    await waitFor(() => {
      expect(getDashboard.mock.calls.length).toBeGreaterThanOrEqual(2);
    });
  });
});
