import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ExportEnvModal } from "./ExportEnvModal";

vi.mock("../../lib/api", () => ({
  exportEnvFiles: vi.fn(async () => ({ fileCount: 3, encrypted: true, cancelled: false })),
  publishTeamChannel: vi.fn(async () => ({ packageId: "fake-package", fileCount: 3 })),
}));

describe("ExportEnvModal", () => {
  beforeEach(() => vi.clearAllMocks());

  it("keeps standard and encrypted exports separate and requires matching passphrases", async () => {
    const user = userEvent.setup();
    render(<ExportEnvModal projectId="demo" projection={demoProjection} onClose={vi.fn()} onError={vi.fn()} onNotice={vi.fn()} />);

    expect(screen.getByText("Standard export")).toBeInTheDocument();
    expect(screen.getByText("Encrypted export")).toBeInTheDocument();
    const action = screen.getByRole("button", { name: "Export" });
    expect(action).toBeDisabled();
    await user.type(screen.getByLabelText("Passphrase"), "fake-passphrase-2026");
    await user.type(screen.getByLabelText("Confirm passphrase"), "different-passphrase");
    expect(screen.getByText("The passphrases do not match.")).toBeInTheDocument();
    expect(action).toBeDisabled();

    await user.clear(screen.getByLabelText("Confirm passphrase"));
    await user.type(screen.getByLabelText("Confirm passphrase"), "fake-passphrase-2026");
    await user.click(action);
    expect(api.exportEnvFiles).toHaveBeenCalledWith("demo", "fake-passphrase-2026", null, "en");
  });

  it("selects every member of an explicit link for partial sharing", async () => {
    const user = userEvent.setup();
    render(<ExportEnvModal projectId="demo" projection={demoProjection} onClose={vi.fn()} onError={vi.fn()} onNotice={vi.fn()} />);

    await user.click(screen.getByText("Choose what to share"));
    const apiKeyBoxes = screen.getAllByText("GPT_API_KEY").map((node) => node.closest("label")?.querySelector("input"));
    await user.click(apiKeyBoxes[0]!);
    expect(apiKeyBoxes.every((box) => box?.checked)).toBe(true);
  });

  it("publishes only an encrypted package when a team channel is selected", async () => {
    const user = userEvent.setup();
    render(<ExportEnvModal projectId="demo" channelId="channel-demo" projection={demoProjection} onClose={vi.fn()} onError={vi.fn()} onNotice={vi.fn()} />);

    expect(screen.queryByText("Standard export")).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("Passphrase"), "fake-passphrase-2026");
    await user.type(screen.getByLabelText("Confirm passphrase"), "fake-passphrase-2026");
    await user.click(screen.getByRole("button", { name: "Publish" }));

    expect(api.publishTeamChannel).toHaveBeenCalledWith(
      "demo",
      "channel-demo",
      "fake-passphrase-2026",
      null,
    );
    expect(api.exportEnvFiles).not.toHaveBeenCalled();
  });
});
