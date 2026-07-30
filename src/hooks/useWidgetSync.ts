import { useEffect, useRef } from "react";
import { setWidgetConfig, reloadAllTimelines } from "tauri-plugin-widgets-api";
import type { DashboardSnapshot } from "@/types";
import { buildWidgetConfig, WIDGET_GROUP } from "@/lib/widgets";

// Push the dashboard snapshot to the native widget when it changes.
// - Throttles to one push per WIDGET_MIN_INTERVAL_MS to avoid burning
//   platform reload budgets (iOS/macOS WidgetKit rate-limits, Android
//   AppWidget broadcast traffic).
// - Silently no-ops outside the Tauri runtime so Vite dev / SSR / tests
//   don't error.
const WIDGET_MIN_INTERVAL_MS = 60_000;

export function useWidgetSync(snapshot: DashboardSnapshot | undefined) {
  const lastPushedRef = useRef<number>(0);
  const inFlightRef = useRef<Promise<void> | null>(null);

  useEffect(() => {
    if (!snapshot) return;
    if (typeof window === "undefined") return;
    if (!("__TAURI_INTERNALS__" in window)) return;

    const now = Date.now();
    if (now - lastPushedRef.current < WIDGET_MIN_INTERVAL_MS) return;

    const config = buildWidgetConfig(snapshot);
    const run = async () => {
      try {
        await setWidgetConfig(config, WIDGET_GROUP);
        await reloadAllTimelines();
      } catch (err) {
        // Widget extension may not be registered on this platform
        // (e.g. dev builds without a widget extension target). Swallow.
        console.warn("[widget-sync] push failed:", err);
      }
    };

    lastPushedRef.current = now;
    inFlightRef.current = run().finally(() => {
      inFlightRef.current = null;
    });
  }, [snapshot]);
}
