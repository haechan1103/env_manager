import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { AgentActivity } from "./AgentActivity";

vi.mock("../../lib/api", () => ({
  listAgentActivity: vi.fn(async () => [{
    timestampMs: 1,
    projectId: "demo",
    actor: "claude-code",
    category: "value-read",
    operation: "read_allowed_value",
    relativePaths: [".env.local"],
    variableNames: ["GPT_API_KEY"],
    policyDecision: "policy-checked",
    outcome: "blocked",
    resultCode: "CODEX_ACCESS_BLOCKED",
  }]),
}));

describe("AgentActivity", () => {
  it("shows allowlisted targets and never asks the API for values", async () => {
    render(<AgentActivity projectId="demo" onError={vi.fn()} />);
    expect(await screen.findByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("Value read attempt")).toBeInTheDocument();
    expect(screen.getByText("GPT_API_KEY")).toBeInTheDocument();
    expect(screen.getByText("Blocked")).toBeInTheDocument();
    expect(api.listAgentActivity).toHaveBeenCalledWith("demo");
    expect(screen.queryByText("fake_preview_value")).not.toBeInTheDocument();
  });
});
