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
    installedVersion: "0.3.0",
    currentVersion: "0.3.0",
    updateAvailable: false,
    protection: "broker",
    detail: "Redacted broker가 연결되어 있습니다.",
    canInstall: true,
  },
  {
    id: "claude-code",
    name: "Claude Code",
    detected: true,
    installed: false,
    installedVersion: null,
    currentVersion: "0.3.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "연동을 설치할 수 있습니다.",
    canInstall: true,
  },
  {
    id: "github-copilot",
    name: "GitHub Copilot / VS Code",
    detected: false,
    installed: false,
    installedVersion: null,
    currentVersion: "0.3.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "도구가 설치되면 연동할 수 있습니다.",
    canInstall: false,
  },
];

vi.mock("../../lib/api", () => ({
  listAgentIntegrations: vi.fn(async () => integrations),
  installAgentIntegration: vi.fn(async (id: string) => ({
    ...integrations.find((item) => item.id === id)!,
    installed: true,
    installedVersion: "0.3.0",
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
    await user.click(within(claudeCard!).getByRole("button", { name: "연결 설치" }));

    expect(api.installAgentIntegration).toHaveBeenCalledWith("claude-code");
    expect(onNotice).toHaveBeenCalledWith(expect.stringContaining("Claude Code"));
  });
});
