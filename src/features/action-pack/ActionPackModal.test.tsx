import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ActionPackModal } from "./ActionPackModal";

vi.mock("../../lib/api", () => ({
  listActionPacks: vi.fn(async () => [{
    id: "local.demo.api-check",
    displayName: "API health check",
    description: "Fixed endpoint",
    packVersion: "1.0.0",
    kind: "http",
    available: true,
    bindings: [{ id: "Authorization", destination: "Authorization" }],
    target: "https://api.example.com/health",
    cliVersion: null,
    profileId: null,
  }]),
  chooseAndInstallActionPack: vi.fn(async () => null),
  removeActionPack: vi.fn(async () => undefined),
  executeActionPack: vi.fn(async (_projectId: string, request: { packId: string }) => ({
    packId: request.packId,
    kind: "http",
    succeeded: true,
    statusCode: 200,
    durationMs: 42,
    exitCode: null,
    resultCode: "ACTION_SUCCEEDED",
  })),
}));

describe("ActionPackModal", () => {
  beforeEach(() => vi.clearAllMocks());

  it("maps variable names only and renders only allowlisted result metadata", async () => {
    const user = userEvent.setup();
    render(
      <ActionPackModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    await waitFor(() => expect(api.listActionPacks).toHaveBeenCalledWith("demo-project"));
    await user.selectOptions(screen.getByLabelText(/Authorization/), "GPT_API_KEY");
    await user.click(screen.getByRole("button", { name: "Run action" }));

    await waitFor(() => expect(api.executeActionPack).toHaveBeenCalledWith(
      "demo-project",
      {
        packId: "local.demo.api-check",
        file: ".env.local",
        bindings: { Authorization: "GPT_API_KEY" },
      },
    ));
    expect(screen.getByText("HTTP 200")).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();
    expect(JSON.stringify(vi.mocked(api.executeActionPack).mock.calls)).not.toContain("fake_");
  });
});
