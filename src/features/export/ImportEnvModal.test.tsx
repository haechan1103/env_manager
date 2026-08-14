import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ImportEnvModal } from "./ImportEnvModal";

vi.mock("../../lib/api", () => ({
  planTeamImport: vi.fn(async () => ({
    planId: "fake-plan",
    expiresInSeconds: 300,
    preview: {
      files: [
        { path: ".env.local", targetPath: ".env.local", occurrences: [{ id: "conflict-1", key: "TOKEN", state: "conflict", linkId: null }] },
        { path: "apps/web/.env.dev", targetPath: "apps/web/.env.dev", occurrences: [{ id: "new-1", key: "PORT", state: "new", linkId: null }] },
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
  remapTeamImportFile: vi.fn(async () => ({
    files: [
      { path: ".env.local", targetPath: ".env.development", occurrences: [{ id: "conflict-1", key: "TOKEN", state: "new", linkId: null }] },
      { path: "apps/web/.env.dev", targetPath: "apps/web/.env.dev", occurrences: [{ id: "new-1", key: "PORT", state: "new", linkId: null }] },
    ],
    newCount: 2,
    unchangedCount: 0,
    conflictCount: 0,
  })),
  revealTeamImportConflict: vi.fn(async (_projectId: string, _planId: string, _occurrenceId: string, side: string) => side === "local" ? "fake_local_value" : "fake_shared_value"),
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
        projection={demoProjection}
        onApplied={onApplied}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={onNotice}
      />,
    );

    const passphrase = "fake-team-passphrase-2026";
    await user.type(screen.getByLabelText("Share passphrase"), passphrase);
    await user.click(screen.getByRole("button", { name: "Choose encrypted file" }));

    expect(await screen.findByLabelText("Target file for .env.local")).toHaveValue(".env.local");
    expect(screen.getByText("TOKEN")).toBeInTheDocument();
    expect(screen.queryByDisplayValue(passphrase)).not.toBeInTheDocument();
    expect(document.body.textContent).not.toContain(passphrase);
    await user.click(screen.getByRole("button", { name: "Use shared" }));
    await user.click(screen.getByRole("button", { name: "Apply to project" }));

    expect(api.applyTeamImport).toHaveBeenCalledWith("demo", "fake-plan", ["conflict-1"]);
    expect(onApplied).toHaveBeenCalled();
    expect(onNotice).toHaveBeenCalledWith("Applied: 1 added, 1 updated, 0 kept local, 0 unchanged.");
  });

  it("reveals only the requested conflict side and can remap an incoming file", async () => {
    const user = userEvent.setup();
    render(
      <ImportEnvModal
        projectId="demo"
        projection={demoProjection}
        onApplied={vi.fn(async () => undefined)}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Share passphrase"), "fake-team-passphrase-2026");
    await user.click(screen.getByRole("button", { name: "Choose encrypted file" }));
    await user.click(await screen.findByRole("button", { name: "Reveal my local value" }));

    expect(await screen.findByText("fake_local_value")).toBeInTheDocument();
    expect(screen.queryByText("fake_shared_value")).not.toBeInTheDocument();
    expect(api.revealTeamImportConflict).toHaveBeenCalledWith("demo", "fake-plan", "conflict-1", "local");

    const target = screen.getByLabelText("Target file for .env.local");
    await user.clear(target);
    await user.type(target, ".env.development");
    await user.click(screen.getAllByRole("button", { name: "Change" })[0]!);
    expect(api.remapTeamImportFile).toHaveBeenCalledWith("demo", "fake-plan", ".env.local", ".env.development");
    expect(await screen.findByText("No conflicting values")).toBeInTheDocument();
  });
});
