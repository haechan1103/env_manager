import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import * as api from "../../lib/api";
import type { AgentIntegrationStatus } from "../../lib/types";
import {
  INITIAL_UPDATE_CHECK_DELAY_MS,
  UPDATE_CHECK_INTERVAL_MS,
} from "../updater/checkSchedule";

interface AgentIntegrationStatusContextValue {
  items: AgentIntegrationStatus[];
  loading: boolean;
  needsAttention: boolean;
  refresh: () => Promise<AgentIntegrationStatus[]>;
  replace: (item: AgentIntegrationStatus) => void;
}

const AgentIntegrationStatusContext =
  createContext<AgentIntegrationStatusContextValue | null>(null);

export function agentIntegrationNeedsAttention(item: AgentIntegrationStatus) {
  return item.detected
    && item.actionBlocker !== "tool-not-found"
    && (!item.installed || item.updateAvailable || item.needsRepair);
}

export function AgentIntegrationStatusProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<AgentIntegrationStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const pendingRef = useRef<Promise<AgentIntegrationStatus[]> | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(() => {
    if (pendingRef.current) return pendingRef.current;

    if (mountedRef.current) setLoading(true);
    const pending = api.listAgentIntegrations()
      .then((next) => {
        if (mountedRef.current) setItems(next);
        return next;
      })
      .finally(() => {
        pendingRef.current = null;
        if (mountedRef.current) setLoading(false);
      });
    pendingRef.current = pending;
    return pending;
  }, []);

  const replace = useCallback((item: AgentIntegrationStatus) => {
    setItems((current) => current.map((entry) => (entry.id === item.id ? item : entry)));
  }, []);

  useEffect(() => {
    const checkQuietly = () => void refresh().catch(() => undefined);
    const initialTimer = window.setTimeout(checkQuietly, INITIAL_UPDATE_CHECK_DELAY_MS);
    const interval = window.setInterval(checkQuietly, UPDATE_CHECK_INTERVAL_MS);
    return () => {
      window.clearTimeout(initialTimer);
      window.clearInterval(interval);
    };
  }, [refresh]);

  const value = useMemo<AgentIntegrationStatusContextValue>(() => ({
    items,
    loading,
    needsAttention: items.some(agentIntegrationNeedsAttention),
    refresh,
    replace,
  }), [items, loading, refresh, replace]);

  return (
    <AgentIntegrationStatusContext.Provider value={value}>
      {children}
    </AgentIntegrationStatusContext.Provider>
  );
}

export function useAgentIntegrationStatus() {
  const context = useContext(AgentIntegrationStatusContext);
  if (!context) {
    throw new Error("AgentIntegrationStatusProvider is missing");
  }
  return context;
}
