import {
  DashboardSnapshot,
  HistoryPoint,
  PublicAccount,
  AccountInput,
  CredentialProbe,
  Settings,
  DeviceAuthStatus,
  AccountDashboard,
} from "@/types";

export interface DeviceFlowStartResult {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

// ── In-Memory Demo State ─────────────────────────────────────────

let mockSettings: Settings = {
  refreshIntervalMinutes: 5,
};

let mockAccounts: PublicAccount[] = [
  {
    id: "cli:claude",
    provider: "claude",
    displayName: "Claude (CLI)",
    enabled: true,
    credentialSource: "cli_auto",
    hasCredential: true,
    config: { type: "claude" },
  },
  {
    id: "gemini:1",
    provider: "gemini",
    displayName: "Gemini Pro",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "gemini" },
  },
  {
    id: "kimi:1",
    provider: "kimi",
    displayName: "Kimi Personal",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "kimi" },
  },
  {
    id: "codex:1",
    provider: "codex",
    displayName: "Codex Free",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "codex" },
  },
  {
    id: "minimax:1",
    provider: "minimax",
    displayName: "MiniMax CN",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "minimax", region: "cn" },
  },
  {
    id: "copilot:1",
    provider: "copilot",
    displayName: "Copilot Business",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "copilot", githubDomain: null },
  },
  {
    id: "volcengine:1",
    provider: "volcengine",
    displayName: "Volcengine Ark",
    enabled: true,
    credentialSource: "env",
    hasCredential: false,
    config: { type: "volcengine", region: "cn-beijing" },
  },
];

let refreshCount = 0;
let copilotPolls = 0;

// Helper to generate history
function generateMockHistory(
  utilizationStart: number,
  utilizationEnd: number,
  hours: number
): HistoryPoint[] {
  const points: HistoryPoint[] = [];
  const now = Date.now();
  const step = (hours * 60 * 60 * 1000) / 24;
  for (let i = 0; i <= 24; i++) {
    const t = now - (24 - i) * step;
    const progress = i / 24;
    const val = utilizationStart + progress * (utilizationEnd - utilizationStart);
    points.push({
      sampledAt: t,
      utilization: val,
      resetsAt: now + 3 * 60 * 60 * 1000,
    });
  }
  return points;
}

// ── Mock Implementation ──────────────────────────────────────────

export async function getDashboard(): Promise<DashboardSnapshot> {
  const now = Date.now();
  const resetIn3h = now + 3 * 60 * 60 * 1000;
  
  const accounts: AccountDashboard[] = mockAccounts
    .filter((a) => a.enabled)
    .map((a) => {
      // Different mock configurations for each provider
      if (a.id === "cli:claude") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: false,
          error: null,
          tiers: [
            {
              quota: {
                id: "five_hour",
                label: "5-Hour Session",
                utilization: 92.5,
                resetsAt: resetIn3h,
                unlimited: false,
              },
              forecast: {
                state: "at_risk",
                ratePerHour: 12.0,
                projectedUtilizationAtReset: 100.0,
                exhaustionAt: now + 45 * 60 * 1000, // 45 mins
                sampleCount: 15,
                observationMinutes: 90,
              },
            },
            {
              quota: {
                id: "seven_day",
                label: "7-Day Limit",
                utilization: 34.0,
                resetsAt: now + 4 * 24 * 60 * 60 * 1000,
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
            },
            {
              quota: {
                id: "monthly",
                label: "Monthly Limit",
                utilization: 12.4,
                resetsAt: now + 21 * 24 * 60 * 60 * 1000,
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
            },
          ],
        };
      } else if (a.id === "gemini:1") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: false,
          error: null,
          tiers: [
            {
              quota: {
                id: "gemini_pro",
                label: "Gemini Pro",
                utilization: 15.0,
                resetsAt: resetIn3h,
                unlimited: false,
              },
              forecast: {
                state: "safe",
                ratePerHour: 2.0,
                projectedUtilizationAtReset: 21.0,
                exhaustionAt: now + 42.5 * 60 * 60 * 1000,
                sampleCount: 12,
                observationMinutes: 60,
              },
            },
            {
              quota: {
                id: "gemini_flash",
                label: "Gemini Flash",
                utilization: 63.0,
                resetsAt: now + 9 * 60 * 60 * 1000,
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
            },
          ],
        };
      } else if (a.id === "kimi:1") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: false,
          error: null,
          tiers: [
            {
              quota: {
                id: "five_hour",
                label: "5-Hour Limit",
                utilization: 45.0,
                resetsAt: now + 5 * 60 * 60 * 1000,
                unlimited: false,
              },
              forecast: {
                state: "learning",
                ratePerHour: 0.0,
                projectedUtilizationAtReset: 45.0,
                exhaustionAt: null,
                sampleCount: 2, // < 3 samples
                observationMinutes: 10,
              },
            },
            {
              quota: {
                id: "weekly_limit",
                label: "Weekly Limit",
                utilization: 58.0,
                resetsAt: now + 3 * 24 * 60 * 60 * 1000,
                used: 290,
                limit: 500,
                unit: "requests",
                unlimited: false,
              },
              forecast: {
                state: "safe",
                ratePerHour: 0.4,
                projectedUtilizationAtReset: 66.0,
                exhaustionAt: null,
                sampleCount: 30,
                observationMinutes: 720,
              },
            },
          ],
        };
      } else if (a.id === "copilot:1") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: false,
          error: null,
          tiers: [
            {
              quota: {
                id: "monthly",
                label: "Monthly Limit",
                utilization: 0.0,
                resetsAt: now + 18 * 24 * 60 * 60 * 1000,
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
          ],
        };
      } else if (a.id === "codex:1") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: false,
          error: null,
          tiers: [
            {
              quota: {
                id: "30_day",
                label: "30-Day Limit",
                utilization: 65.0,
                resetsAt: null, // unknown_reset
                unlimited: false,
              },
              forecast: {
                state: "unknown_reset",
                ratePerHour: 1.5,
                projectedUtilizationAtReset: 65.0,
                exhaustionAt: null,
                sampleCount: 20,
                observationMinutes: 120,
              },
            },
          ],
        };
      } else if (a.id === "minimax:1") {
        return {
          account: a,
          credentialStatus: "valid",
          stale: true, // stale result
          error: "Timeout reading MiniMax remains API",
          tiers: [
            {
              quota: {
                id: "five_hour",
                label: "5-Hour Limit",
                utilization: 30.0,
                resetsAt: resetIn3h,
                unlimited: false,
              },
              forecast: {
                state: "safe",
                ratePerHour: 0.0,
                projectedUtilizationAtReset: 30.0,
                exhaustionAt: null,
                sampleCount: 5,
                observationMinutes: 25,
              },
            },
          ],
        };
      } else {
        // volcengine:1
        return {
          account: a,
          credentialStatus: "expired", // credential error
          stale: false,
          error: "Authentication failed: AccessKey ID / Secret are incorrect.",
          tiers: [],
        };
      }
    });

  // Calculate leading risk
  const severity = (state: string) => {
    switch (state) {
      case "exhausted": return 5;
      case "at_risk": return 4;
      case "unknown_reset": return 3;
      case "learning": return 2;
      case "safe": return 1;
      default: return 0;
    }
  };
  
  let leadingRisk: RiskState = "safe";
  for (const acc of accounts) {
    for (const tier of acc.tiers) {
      if (severity(tier.forecast.state) > severity(leadingRisk)) {
        leadingRisk = tier.forecast.state;
      }
    }
  }

  return {
    accounts,
    leadingRisk,
    refreshedAt: now - 30 * 1000,
    nextRefreshAt: now + mockSettings.refreshIntervalMinutes * 60 * 1000,
    refreshInProgress: false,
  };
}

export async function getTierHistory(
  accountId: string,
  tierId: string,
  hours: number
): Promise<HistoryPoint[]> {
  if (accountId === "cli:claude") {
    return generateMockHistory(40.0, 92.5, hours);
  } else if (accountId === "gemini:1") {
    return generateMockHistory(10.0, 15.0, hours);
  } else if (accountId === "kimi:1") {
    return generateMockHistory(45.0, 45.0, hours);
  } else {
    return generateMockHistory(20.0, 65.0, hours);
  }
}

export async function refreshAll(): Promise<DashboardSnapshot> {
  refreshCount += 1;
  const { promise, resolve } = Promise.withResolvers<DashboardSnapshot>();
  setTimeout(async () => {
    resolve(await getDashboard());
  }, 1000); // Simulate network latency
  return promise;
}

export async function listAccounts(): Promise<PublicAccount[]> {
  return [...mockAccounts];
}

export async function saveAccount(input: AccountInput): Promise<PublicAccount> {
  const isNew = !input.id;
  const accountId = input.id || `manual:${input.config.type}:${Math.random().toString(36).substr(2, 9)}`;
  
  const acc: PublicAccount = {
    id: accountId,
    provider: input.config.type as ProviderKind,
    displayName: input.displayName,
    enabled: input.enabled,
    credentialSource: "env",
    hasCredential: input.secret ? true : !isNew,
    config: input.config,
  };

  if (isNew) {
    mockAccounts.push(acc);
  } else {
    mockAccounts = mockAccounts.map((a) => (a.id === accountId ? acc : a));
  }

  return acc;
}

export async function deleteAccount(accountId: string): Promise<void> {
  mockAccounts = mockAccounts.filter((a) => a.id !== accountId);
}

export async function probeCliCredentials(): Promise<CredentialProbe[]> {
  return [
    {
      provider: "claude",
      status: "valid",
      accountId: "cli:claude",
    },
    {
      provider: "codex",
      status: "valid",
      accountId: "cli:codex",
    },
    {
      provider: "gemini",
      status: "valid",
      accountId: "cli:gemini",
    },
  ];
}

export async function startCopilotDeviceFlow(
  githubDomain?: string | null
): Promise<DeviceFlowStartResult> {
  copilotPolls = 0;
  return {
    deviceCode: "mock_device_code_123",
    userCode: "ABCD-EFGH",
    verificationUri: "https://github.com/login/device",
    expiresIn: 900,
    interval: 5,
  };
}

export async function pollCopilotDeviceFlow(
  deviceCode: string,
  githubDomain?: string | null
): Promise<DeviceAuthStatus> {
  copilotPolls += 1;
  
  if (copilotPolls < 3) {
    return { type: "pending", retryAfterSeconds: 5 };
  }
  
  // Successful link on 3rd poll
  const newAccount: PublicAccount = {
    id: "copilot:12345",
    provider: "copilot",
    displayName: "Copilot (mockuser)",
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "copilot", githubDomain },
  };

  mockAccounts.push(newAccount);

  return { type: "authorized", account: newAccount };
}

export async function getSettings(): Promise<Settings> {
  return { ...mockSettings };
}

export async function updateSettings(input: Settings): Promise<void> {
  mockSettings = { ...input };
}

export async function clearHistory(): Promise<void> {
  // Clear mock history (no op for demo)
}

export async function openMainWindow(): Promise<void> {
  // Deterministic no-op in demo mode
}
export async function discoverEnvAccounts(): Promise<PublicAccount[]> {
  const newAccounts: PublicAccount[] = [
    {
      id: "kimi:env-discovered",
      provider: "kimi",
      displayName: "Kimi (Env)",
      enabled: true,
      credentialSource: "env",
      hasCredential: true,
      config: { type: "kimi" },
    },
  ];
  const existing = mockAccounts.map((a) => a.id);
  const created: PublicAccount[] = [];
  for (const acc of newAccounts) {
    if (!existing.includes(acc.id)) {
      mockAccounts.push(acc);
      created.push(acc);
    }
  }
  return created;
}
