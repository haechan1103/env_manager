import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { ImportEnvModal } from "./ImportEnvModal";

vi.mock("../../lib/api", () => ({
  planTeamImport: vi.fn(async () => ({
    planId: "fake-plan",
    expiresInSeconds: 300,
    preview: {
      files: [
        { path: ".env.local", occurrences: [{ id: "conflict-1", key: "TOKEN", state: "conflict", linkId: null }] },
        { path: "apps/web/.env.dev", occurrences: [{ id: "new-1", key: "PORT", state: "new", linkId: null }] },
      ],
      newCount: 1,
      unchangedCount: 0,
      conflictCount: 1,
    },
  })),
  applyTeamImport: vi.fn(async () => ({
    addedCount: 1,
    updatedCount: 1,
    unchangedCount: 0,
    keptLocalCount: 0,
    affectedFiles: [".env.local", "apps/web/.env.dev"],
  })),
  discardTeamImport: vi.fn(async () => undefined),
}));

describe("ImportEnvModal", () => {
  it("keeps passphrases out of the redacted preview and requires explicit conflict replacement", async () => {
    const user = userEvent.setup();
    const onApplied = vi.fn(async () => undefined);
    const onNotice = vi.fn();
    render(
      <ImportEnvModal
        projectId="demo"
        onApplied={onApplied}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={onNotice}
      />,
    );

    const passphrase = "fake-team-passphrase-2026";
    await user.type(screen.getByLabelText("Share passphrase"), passphrase);
    await user.click(screen.getByRole("button", { name: "Choose encrypted file" }));

    expect(await screen.findByText(".env.local")).toBeInTheDocument();
    expect(screen.getByText("TOKEN")).toBeInTheDocument();
    expect(screen.queryByDisplayValue(passphrase)).not.toBeInTheDocument();
    expect(document.body.textContent).not.toContain(passphrase);
    await user.click(screen.getByLabelText("Use shared"));
    await user.click(screen.getByRole("button", { name: "Apply to project" }));

    expect(api.applyTeamImport).toHaveBeenCalledWith("demo", "fake-plan", ["conflict-1"]);
    expect(onApplied).toHaveBeenCalled();
    expect(onNotice).toHaveBeenCalledWith("Applied: 1 added, 1 updated, 0 kept local, 0 unchanged.");
  });
});
