export type ProviderKind =
  | "claude"
  | "codex"
  | "gemini"
  | "copilot"
  | "kimi"
  | "zhipu"
  | "zhipu_team"
  | "minimax"
  | "zenmux"
  | "volcengine";

export type CredentialSource = "cli_auto" | "env";

export type CredentialStatus =
  | "valid"
  | "expired"
  | "not_found"
  | "parse_error"
  | "unavailable";

export type RiskState =
  | "safe"
  | "at_risk"
  | "exhausted"
  | "learning"
  | "unknown_reset"
  | "error";

export interface QuotaTier {
  id: string;
  label: string;
  utilization: number;
  resetsAt: number | null;
  used?: number | null;
  limit?: number | null;
  unit?: string | null;
  unlimited: boolean;
}

export interface ProviderQuota {
  provider: ProviderKind;
  plan: string;
  tiers: QuotaTier[];
  queriedAt: number;
}

export interface TierForecast {
  state: RiskState;
  ratePerHour: number;
  projectedUtilizationAtReset: number;
  exhaustionAt: number | null;
  sampleCount: number;
  observationMinutes: number;
}

export interface TierDashboard {
  quota: QuotaTier;
  forecast: TierForecast;
}

export interface HistoryPoint {
  sampledAt: number;
  utilization: number;
  resetsAt: number | null;
}

export type ProviderConfig =
  | { type: "claude" }
  | { type: "codex" }
  | { type: "gemini" }
  | { type: "copilot"; githubDomain?: string | null }
  | { type: "kimi" }
  | { type: "zhipu"; region: string }
  | { type: "zhipu_team"; organizationId: string; projectId: string }
  | { type: "minimax"; region: string }
  | { type: "zenmux"; quotaUrl: string }
  | { type: "volcengine"; region: string };

export interface AlertRule {
  tierId: string;
  enabled: boolean;
  thresholdPercent: number;
}

export interface PublicAccount {
  id: string;
  provider: ProviderKind;
  displayName: string;
  enabled: boolean;
  credentialSource: CredentialSource;
  hasCredential: boolean;
  config: ProviderConfig;
  alertRules: AlertRule[];
}

export interface AccountDashboard {
  account: PublicAccount;
  credentialStatus: CredentialStatus;
  stale: boolean;
  error: string | null;
  tiers: TierDashboard[];
}

export interface DashboardSnapshot {
  accounts: AccountDashboard[];
  leadingRisk: RiskState;
  refreshedAt: number;
  nextRefreshAt: number;
  refreshInProgress: boolean;
}

export type SecretPayload =
  | { type: "api_key"; apiKey: string }
  | { type: "volcengine"; accessKeyId: string; secretAccessKey: string };

export interface AccountInput {
  id?: string;
  displayName: string;
  enabled: boolean;
  config: ProviderConfig;
  secret?: SecretPayload;
  removeCredential?: boolean;
  alertRules?: AlertRule[];
}

export interface Settings {
  refreshIntervalMinutes: number;
  /** Manual overview card order. Empty means sort by risk severity. */
  accountOrder: string[];
}

export interface CredentialProbe {
  provider: ProviderKind;
  status: CredentialStatus;
  message?: string | null;
  accountId?: string | null;
}

export type DeviceAuthStatus =
  | { type: "pending"; retryAfterSeconds: number }
  | { type: "authorized"; account: PublicAccount }
  | { type: "expired" }
  | { type: "denied"; message: string };

export interface AppError {
  code: string;
  message: string;
  retryable: boolean;
}
