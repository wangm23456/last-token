import * as React from "react";
import { useQuery, useMutation, useQueryClient, type QueryClient, type UseMutationResult, type UseQueryResult } from "@tanstack/react-query";
import { toast } from "sonner";

import { getDashboard, refreshAll } from "@/lib/backend";
import { getErrorMessage } from "@/lib/errors";
import type { DashboardSnapshot } from "@/types";

const DASHBOARD_QUERY_KEY = ["dashboard"] as const;

export interface UseDashboardSnapshotResult {
  query: UseQueryResult<DashboardSnapshot, Error>;
  refreshMutation: UseMutationResult<DashboardSnapshot, Error, void, unknown>;
  queryClient: QueryClient;
}

function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  const globalScope = window as unknown as Record<string, unknown>;
  return "__TAURI_INTERNALS__" in globalScope;
}

export function useDashboardSnapshot(): UseDashboardSnapshotResult {
  const queryClient = useQueryClient();

  const query = useQuery<DashboardSnapshot, Error>({
    queryKey: DASHBOARD_QUERY_KEY,
    queryFn: getDashboard,
    refetchInterval: 60 * 1000,
  });

  const refreshMutation = useMutation<DashboardSnapshot, Error, void, unknown>({
    mutationFn: refreshAll,
    onSuccess: (data) => {
      queryClient.setQueryData(DASHBOARD_QUERY_KEY, data);
      toast.success("刷新成功");
    },
    onError: (err) => {
      const message = getErrorMessage(err);
      toast.error(`刷新失败: ${message}`);
    },
  });

  React.useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<DashboardSnapshot | null>("quota-updated", (event) => {
          if (cancelled) {
            return;
          }
          if (event.payload) {
            queryClient.setQueryData(DASHBOARD_QUERY_KEY, event.payload);
          } else {
            void queryClient.invalidateQueries({ queryKey: DASHBOARD_QUERY_KEY });
          }
          // Keep settings fresh across surfaces (main + tray webviews).
          void queryClient.invalidateQueries({ queryKey: ["settings"] });
        }),
      )
      .then((dispose) => {
        if (cancelled) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => {
        // Tauri internals advertised but event module failed to load — degrade silently.
      });

    return () => {
      cancelled = true;
      if (unlisten) {
        try {
          unlisten();
        } catch {
          // ignore double-cleanup
        }
      }
    };
  }, [queryClient]);

  return {
    query,
    refreshMutation,
    queryClient,
  };
}
