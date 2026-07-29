import { riskSeverity, worstTier } from "@/components/QuotaTierList";
import type { AccountDashboard } from "@/types";

/** Higher severity first; same severity uses earliest exhaustion/reset. */
export function sortAccountsByRisk(accounts: AccountDashboard[]): AccountDashboard[] {
  const accountSeverity = (acc: AccountDashboard): number => {
    if (acc.credentialStatus !== "valid" || acc.error) {
      // Account-level error: place between unknown_reset and learning.
      return riskSeverity("unknown_reset") + 0.5;
    }
    const w = worstTier(acc.tiers);
    return w ? riskSeverity(w.forecast.state) : riskSeverity("safe");
  };

  const earliestTime = (acc: AccountDashboard): number => {
    let min = Number.POSITIVE_INFINITY;
    for (const t of acc.tiers) {
      const t1 = t.forecast.exhaustionAt ?? t.quota.resetsAt;
      if (t1 && t1 < min) min = t1;
    }
    return min;
  };

  return [...accounts].sort((a, b) => {
    const sevA = accountSeverity(a);
    const sevB = accountSeverity(b);
    if (sevA !== sevB) return sevB - sevA;
    return earliestTime(a) - earliestTime(b);
  });
}

/**
 * Manual order wins when present; otherwise sort by risk severity.
 * Unknown/new ids not in `order` are appended in risk order.
 */
export function applyAccountOrder(
  accounts: AccountDashboard[],
  order: string[] | undefined,
): AccountDashboard[] {
  const byRisk = sortAccountsByRisk(accounts);
  if (!order || order.length === 0) return byRisk;

  const remaining = new Map(accounts.map((acc) => [acc.account.id, acc]));
  const ordered: AccountDashboard[] = [];
  for (const id of order) {
    const acc = remaining.get(id);
    if (!acc) continue;
    ordered.push(acc);
    remaining.delete(id);
  }
  for (const acc of byRisk) {
    if (remaining.has(acc.account.id)) ordered.push(acc);
  }
  return ordered;
}

/**
 * Reorder arbitrary items by preferred id list.
 * Items missing from `order` keep their relative fallback order at the end.
 */
export function applyIdOrder<T>(
  items: T[],
  getId: (item: T) => string,
  order: string[] | undefined,
  fallbackOrder?: string[],
): T[] {
  if (items.length <= 1) return items;

  const preferred =
    order && order.length > 0
      ? order
      : fallbackOrder && fallbackOrder.length > 0
        ? fallbackOrder
        : null;

  if (!preferred) return items;

  const remaining = new Map(items.map((item) => [getId(item), item]));
  const ordered: T[] = [];
  for (const id of preferred) {
    const item = remaining.get(id);
    if (!item) continue;
    ordered.push(item);
    remaining.delete(id);
  }
  for (const item of items) {
    if (remaining.has(getId(item))) {
      ordered.push(item);
      remaining.delete(getId(item));
    }
  }
  return ordered;
}
