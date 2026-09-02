import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { AccountProjection } from "../../lib/types";
import { AccountVault } from "./AccountVault";

vi.mock("../../lib/api", () => ({
  listAccounts: vi.fn(),
  createAccount: vi.fn(),
  updateAccount: vi.fn(),
  deleteAccount: vi.fn(),
  setAccountProjectAccess: vi.fn(),
  copyAccountField: vi.fn(),
}));

const account: AccountProjection = {
  id: "0123456789abcdef0123456789abcdef",
  displayName: "Staging admin",
  service: "staging.example.test",
  allowedForProject: false,
  allowedProjectCount: 0,
  createdAtMs: 1,
  updatedAtMs: 1,
};

describe("AccountVault", () => {
  beforeEach(() => {
    vi.mocked(api.listAccounts).mockResolvedValue([account]);
    vi.mocked(api.setAccountProjectAccess).mockResolvedValue();
    vi.mocked(api.copyAccountField).mockResolvedValue();
    vi.mocked(api.deleteAccount).mockResolvedValue();
    vi.mocked(api.updateAccount).mockResolvedValue();
    vi.mocked(api.createAccount).mockResolvedValue(account);
  });

  it("requires an explicit desktop action before this project can use an account", async () => {
    const user = userEvent.setup();
    render(
      <AccountVault
        projectId="0123456789abcdef"
        projectName="Demo project"
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    expect(await screen.findByText("Staging admin")).toBeInTheDocument();
    expect(screen.getByText("Not allowed")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy password" })).toBeDisabled();
    expect(screen.queryByText("fake-password-canary")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Allow for Demo project" }));

    expect(api.setAccountProjectAccess).toHaveBeenCalledWith(
      "0123456789abcdef",
      account.id,
      true,
    );
    expect(screen.getByText("Allowed for this project")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy password" })).toBeEnabled();
  });

  it("keeps a new account blocked by default", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listAccounts).mockResolvedValue([]);
    render(
      <AccountVault
        projectId="0123456789abcdef"
        projectName="Demo project"
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    await screen.findByText("No accounts saved");
    await user.click(screen.getByRole("button", { name: "+ Add account" }));
    await user.type(screen.getByLabelText("Display name"), "Production admin");
    await user.type(screen.getByLabelText("Service or website"), "prod.example.test");
    await user.type(screen.getByLabelText("Username or ID"), "fake-user-canary");
    await user.type(screen.getByLabelText("Password"), "fake-password-canary");
    expect(screen.getByRole("checkbox", { name: "Allow this account for Demo project" })).not.toBeChecked();

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(api.createAccount).toHaveBeenCalledWith("0123456789abcdef", {
      displayName: "Production admin",
      service: "prod.example.test",
      username: "fake-user-canary",
      password: "fake-password-canary",
      allowCurrentProject: false,
    });
    expect(screen.queryByDisplayValue("fake-password-canary")).not.toBeInTheDocument();
  });
});
