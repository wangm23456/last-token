import { invoke } from "@tauri-apps/api/core";
import {
  DashboardSnapshot,
  HistoryPoint,
  PublicAccount,
  AccountInput,
  CredentialProbe,
  Settings,
  DeviceAuthStatus,
} from "@/types";


function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  const globalScope = window as unknown as Record<string, unknown>;
  return "__TAURI_INTERNALS__" in globalScope;
}

function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    return Promise.reject(
      new Error("当前处于浏览器预览环境，无法调用限制在 Tauri App 内的命令。")
    );
  }
  return invoke<T>(cmd, args);
}


export interface DeviceFlowStartResult {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
}

export async function getDashboard(): Promise<DashboardSnapshot> {
  return safeInvoke<DashboardSnapshot>("get_dashboard");
}

export async function getTierHistory(
  accountId: string,
  tierId: string,
  hours: number
): Promise<HistoryPoint[]> {
  return safeInvoke<HistoryPoint[]>("get_tier_history", {
    accountId,
    tierId,
    hours,
  });
}

export async function refreshAll(): Promise<DashboardSnapshot> {
  return safeInvoke<DashboardSnapshot>("refresh_all");
}

export async function listAccounts(): Promise<PublicAccount[]> {
  return safeInvoke<PublicAccount[]>("list_accounts");
}

export async function saveAccount(input: AccountInput): Promise<PublicAccount> {
  return safeInvoke<PublicAccount>("save_account", { input });
}

export async function deleteAccount(accountId: string): Promise<void> {
  return safeInvoke<void>("delete_account", { accountId });
}

export async function probeCliCredentials(): Promise<CredentialProbe[]> {
  return safeInvoke<CredentialProbe[]>("probe_cli_credentials");
}

export async function discoverEnvAccounts(): Promise<PublicAccount[]> {
  return safeInvoke<PublicAccount[]>("discover_env_accounts");
}

export async function startCopilotDeviceFlow(
  githubDomain?: string | null
): Promise<DeviceFlowStartResult> {
  return safeInvoke<DeviceFlowStartResult>("start_copilot_device_flow", {
    githubDomain,
  });
}

export async function pollCopilotDeviceFlow(
  deviceCode: string,
  githubDomain?: string | null,
  currentInterval?: number,
): Promise<DeviceAuthStatus> {
  return safeInvoke<DeviceAuthStatus>("poll_copilot_device_flow", {
    deviceCode,
    githubDomain,
    currentInterval,
  });
}

export async function getSettings(): Promise<Settings> {
  return safeInvoke<Settings>("get_settings");
}

export async function updateSettings(input: Settings): Promise<void> {
  return safeInvoke<void>("update_settings", { input });
}

export async function clearHistory(): Promise<void> {
  return safeInvoke<void>("clear_history");
}

export async function openMainWindow(): Promise<void> {
  return safeInvoke<void>("open_main_window");
}
