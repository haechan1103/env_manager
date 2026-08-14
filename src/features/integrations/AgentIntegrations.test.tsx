import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { AgentIntegrationStatus } from "../../lib/types";
import { AgentIntegrations } from "./AgentIntegrations";

const integrations: AgentIntegrationStatus[] = [
  {
    id: "codex",
    name: "Codex",
    detected: true,
    installed: true,
    installedVersion: "1.0.0",
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "broker",
    detail: "The redacted broker is connected.",
    canInstall: true,
    actionBlocker: null,
  },
  {
    id: "claude-code",
    name: "Claude Code",
    detected: true,
    installed: false,
    installedVersion: null,
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "The integration can be installed.",
    canInstall: true,
    actionBlocker: null,
  },
  {
    id: "github-copilot",
    name: "GitHub Copilot / VS Code",
    detected: false,
    installed: false,
    installedVersion: null,
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "Install the tool to connect it.",
    canInstall: false,
    actionBlocker: "tool-not-found",
  },
];

vi.mock("../../lib/api", () => ({
  listAgentIntegrations: vi.fn(async () => integrations),
  installAgentIntegration: vi.fn(async (id: string) => ({
    ...integrations.find((item) => item.id === id)!,
    installed: true,
    installedVersion: "1.0.0",
    protection: "guarded",
  })),
}));

describe("AgentIntegrations", () => {
  it("shows all supported hosts without exposing env values", async () => {
    render(<AgentIntegrations onError={vi.fn()} onNotice={vi.fn()} />);

    expect(await screen.findByText("Codex")).toBeInTheDocument();
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("GitHub Copilot / VS Code")).toBeInTheDocument();
    expect(screen.queryByText(/API_KEY=/)).not.toBeInTheDocument();
  });

  it("installs the selected host through the shared integration API", async () => {
    const user = userEvent.setup();
    const onNotice = vi.fn();
    render(<AgentIntegrations onError={vi.fn()} onNotice={onNotice} />);

    const claudeCard = (await screen.findByText("Claude Code")).closest("article");
    expect(claudeCard).not.toBeNull();
    await user.click(within(claudeCard!).getByRole("button", { name: "Install connection" }));

    expect(api.installAgentIntegration).toHaveBeenCalledWith("claude-code");
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("Claude Code"));
  });

  it("explains why an integration action is disabled", async () => {
    render(<AgentIntegrations onError={vi.fn()} onNotice={vi.fn()} />);

    const copilotCard = (await screen.findByText("GitHub Copilot / VS Code")).closest("article");
    expect(copilotCard).not.toBeNull();
    expect(within(copilotCard!).getByText(/CLI was not found/)).toBeInTheDocument();
  });

  it("labels app-linked plugin versions as legacy bundle state", async () => {
    vi.mocked(api.listAgentIntegrations).mockResolvedValueOnce([
      { ...integrations[0]!, installedVersion: "0.5.0", legacyVersion: true, updateAvailable: true },
    ]);
    render(<AgentIntegrations onError={vi.fn()} onNotice={vi.fn()} />);

    expect(await screen.findByText("0.5.0 (legacy app-linked) → 1.0.0")).toBeInTheDocument();
  });
});
