import { describe, it, expect } from "vitest";
import { applyAccountOrder, applyIdOrder, sortAccountsByRisk } from "@/lib/accountOrder";
import type { AccountDashboard, PublicAccount, TierDashboard } from "@/types";

const NOW = Date.now();

function makeTier(state: TierDashboard["forecast"]["state"], utilization = 10): TierDashboard {
  return {
    quota: {
      id: "monthly",
      label: "Monthly",
      utilization,
      resetsAt: NOW + 3600_000,
      unlimited: false,
    },
    forecast: {
      state,
      ratePerHour: 1,
      projectedUtilizationAtReset: utilization,
      exhaustionAt: state === "at_risk" ? NOW + 60_000 : null,
      sampleCount: 3,
      observationMinutes: 30,
    },
  };
}

function makeAcc(
  id: string,
  name: string,
  state: TierDashboard["forecast"]["state"],
): AccountDashboard {
  const account: PublicAccount = {
    id,
    provider: "claude",
    displayName: name,
    enabled: true,
    credentialSource: "env",
    hasCredential: true,
    config: { type: "claude" },
    alertRules: [],
  };
  return {
    account,
    credentialStatus: "valid",
    error: null,
    tiers: [makeTier(state)],
  };
}

describe("accountOrder helpers", () => {
  const safe = makeAcc("safe", "Safe", "safe");
  const risk = makeAcc("risk", "Risk", "at_risk");
  const learning = makeAcc("learn", "Learn", "learning");

  it("sorts by risk when no manual order", () => {
    const ordered = applyAccountOrder([safe, learning, risk], []);
    expect(ordered.map((a) => a.account.id)).toEqual(["risk", "learn", "safe"]);
  });

  it("prefers manual order and appends unknowns by risk", () => {
    const ordered = applyAccountOrder([safe, learning, risk], ["safe", "risk"]);
    expect(ordered.map((a) => a.account.id)).toEqual(["safe", "risk", "learn"]);
  });

  it("applyIdOrder uses fallback order when manual order empty", () => {
    const items = [
      { id: "safe", name: "Safe" },
      { id: "risk", name: "Risk" },
      { id: "learn", name: "Learn" },
    ];
    const fallback = sortAccountsByRisk([safe, learning, risk]).map((a) => a.account.id);
    const ordered = applyIdOrder(items, (x) => x.id, [], fallback);
    expect(ordered.map((x) => x.id)).toEqual(["risk", "learn", "safe"]);
  });
});
