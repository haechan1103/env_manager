import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { AgentIntegrationStatus } from "../../lib/types";
import { UPDATE_CHECK_INTERVAL_MS } from "../updater/checkSchedule";
import {
  AgentIntegrationStatusProvider,
  agentIntegrationNeedsAttention,
  useAgentIntegrationStatus,
} from "./AgentIntegrationStatusProvider";

vi.mock("../../lib/api", () => ({
  listAgentIntegrations: vi.fn(),
}));

const healthy: AgentIntegrationStatus = {
  id: "codex",
  name: "Codex",
  detected: true,
  installed: true,
  installedVersion: "1.7.0",
  legacyVersion: false,
  currentVersion: "1.7.0",
  updateAvailable: false,
  needsRepair: false,
  protection: "broker",
  detail: "Connected",
  canInstall: true,
  actionBlocker: null,
};

function Probe() {
  const { needsAttention } = useAgentIntegrationStatus();
  return <span>{needsAttention ? "attention" : "clear"}</span>;
}

describe("AgentIntegrationStatusProvider", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(api.listAgentIntegrations).mockResolvedValue([healthy]);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("checks after launch and once per hour", async () => {
    vi.mocked(api.listAgentIntegrations)
      .mockResolvedValueOnce([healthy])
      .mockResolvedValueOnce([{ ...healthy, updateAvailable: true }]);

    render(
      <AgentIntegrationStatusProvider>
        <Probe />
      </AgentIntegrationStatusProvider>,
    );

    await act(async () => vi.advanceTimersByTimeAsync(1500));
    expect(api.listAgentIntegrations).toHaveBeenCalledTimes(1);
    expect(screen.getByText("clear")).toBeInTheDocument();

    await act(async () => vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS));
    expect(api.listAgentIntegrations).toHaveBeenCalledTimes(2);
    expect(screen.getByText("attention")).toBeInTheDocument();
  });

  it("excludes hosts whose actual CLI is not installed", () => {
    expect(agentIntegrationNeedsAttention({
      ...healthy,
      id: "claude-code",
      name: "Claude Code",
      detected: false,
      installed: false,
      installedVersion: null,
      protection: "inactive",
      canInstall: false,
      actionBlocker: "tool-not-found",
    })).toBe(false);
    expect(agentIntegrationNeedsAttention({
      ...healthy,
      id: "github-copilot",
      name: "GitHub Copilot / VS Code",
      detected: true,
      installed: false,
      installedVersion: null,
      protection: "inactive",
      canInstall: false,
      actionBlocker: "tool-not-found",
    })).toBe(false);
  });

  it("includes detected hosts with a missing, outdated, or repairable connection", () => {
    expect(agentIntegrationNeedsAttention({ ...healthy, installed: false })).toBe(true);
    expect(agentIntegrationNeedsAttention({ ...healthy, updateAvailable: true })).toBe(true);
    expect(agentIntegrationNeedsAttention({ ...healthy, needsRepair: true })).toBe(true);
    expect(agentIntegrationNeedsAttention({
      ...healthy,
      needsRepair: true,
      canInstall: false,
      actionBlocker: "broker-unavailable",
    })).toBe(true);
  });
});
