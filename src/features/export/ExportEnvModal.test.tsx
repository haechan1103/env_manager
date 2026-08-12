import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { ExportEnvModal } from "./ExportEnvModal";

vi.mock("../../lib/api", () => ({
  exportEnvFiles: vi.fn(async () => ({ fileCount: 3, encrypted: true, cancelled: false })),
}));

describe("ExportEnvModal", () => {
  it("keeps standard and encrypted exports separate and requires matching passphrases", async () => {
    const user = userEvent.setup();
    render(<ExportEnvModal projectId="demo" onClose={vi.fn()} onError={vi.fn()} onNotice={vi.fn()} />);

    expect(screen.getByText("Standard export")).toBeInTheDocument();
    expect(screen.getByText("Encrypted export")).toBeInTheDocument();
    await user.click(screen.getByText("Encrypted export"));

    const action = screen.getByRole("button", { name: "Export" });
    expect(action).toBeDisabled();
    await user.type(screen.getByLabelText("Passphrase"), "fake-passphrase-2026");
    await user.type(screen.getByLabelText("Confirm passphrase"), "different-passphrase");
    expect(screen.getByText("The passphrases do not match.")).toBeInTheDocument();
    expect(action).toBeDisabled();

    await user.clear(screen.getByLabelText("Confirm passphrase"));
    await user.type(screen.getByLabelText("Confirm passphrase"), "fake-passphrase-2026");
    await user.click(action);
    expect(api.exportEnvFiles).toHaveBeenCalledWith("demo", "fake-passphrase-2026", "en");
  });
});
