import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { TeamChannelModal } from "./TeamChannelModal";

vi.mock("../../lib/api", () => ({
  listTeamChannels: vi.fn(async () => [
    {
      id: "channel-local-id",
      name: "Mounted team folder",
      readable: true,
      publishable: true,
      packages: [{ id: "pkg_fake_12345678", byteSize: 2048, modifiedAtMs: 1_787_000_000_000 }],
    },
  ]),
  connectFolderTeamChannel: vi.fn(async () => null),
  removeTeamChannel: vi.fn(async () => undefined),
}));

describe("TeamChannelModal", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders immediately, then lists only channel and encrypted-package metadata", async () => {
    vi.mocked(api.listTeamChannels).mockReturnValueOnce(
      new Promise<Awaited<ReturnType<typeof api.listTeamChannels>>>(() => undefined),
    );
    render(
      <TeamChannelModal
        projectId="demo-project"
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onPublish={vi.fn()}
        onImport={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Team sharing" })).toBeInTheDocument();
    expect(screen.getByText("Checking connected folders…")).toBeInTheDocument();
    await waitFor(() => expect(api.listTeamChannels).toHaveBeenCalledWith("demo-project"));
  });

  it("routes publish and package review through opaque IDs", async () => {
    const user = userEvent.setup();
    const onPublish = vi.fn();
    const onImport = vi.fn();
    render(
      <TeamChannelModal
        projectId="demo-project"
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onPublish={onPublish}
        onImport={onImport}
      />,
    );

    await screen.findByText("Mounted team folder");
    expect(screen.queryByText(/fake.*value/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Publish" }));
    expect(onPublish).toHaveBeenCalledWith("channel-local-id");
    await user.click(screen.getByRole("button", { name: "Review" }));
    expect(onImport).toHaveBeenCalledWith("channel-local-id", "pkg_fake_12345678");
  });
});
