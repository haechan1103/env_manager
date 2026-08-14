import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ProviderPushModal } from "./ProviderPushModal";

vi.mock("../../lib/api", () => ({
  listDeploymentProviders: vi.fn(async () => [
    { id: "github-actions", name: "GitHub Actions", available: true, detail: "ready" },
    { id: "cloudflare-workers", name: "Cloudflare Workers", available: true, detail: "ready" },
  ]),
  listGitHubRepositories: vi.fn(async () => ({ repositories: ["owner/repository"] })),
  detectGitHubRepository: vi.fn(async () => ({ repository: null })),
  listGitHubEnvironments: vi.fn(async (_projectId: string, repository: string) => ({
    repository,
    environments: ["preview", "production"],
  })),
  createGitHubEnvironment: vi.fn(async (_projectId: string, repository: string, environment: string) => ({
    repository,
    environments: ["preview", "production", environment],
  })),
  detectCloudflareTarget: vi.fn(async () => ({
    worker: "demo-worker",
    environments: ["staging", "production"],
    configPath: "wrangler.jsonc",
  })),
  pushToProvider: vi.fn(async (_projectId: string, request: { provider: string; selections: unknown[] }) => ({
    provider: request.provider,
    pushedCount: request.selections.length,
    failedKeys: [],
  })),
}));

describe("ProviderPushModal", () => {
  it("shows names only and sends a redacted selection request", async () => {
    const user = userEvent.setup();
    const onNotice = vi.fn();
    render(
      <ProviderPushModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={onNotice}
      />,
    );

    expect(screen.queryByText("fake_preview_value")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByText("CLI ready")).toHaveLength(2));
    await user.type(screen.getByLabelText("GitHub repository"), "owner/repository");
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    const firstCheckbox = checkboxes.at(0);
    expect(firstCheckbox).toBeDefined();
    await user.click(firstCheckbox!);
    await user.click(screen.getByRole("button", { name: "Push 1" }));

    expect(api.pushToProvider).toHaveBeenCalledWith(
      "demo-project",
      expect.objectContaining({
        provider: "github-actions",
        repository: "owner/repository",
        selections: [expect.objectContaining({ kind: "secret" })],
      }),
    );
    const request = vi.mocked(api.pushToProvider).mock.calls[0]?.[1];
    expect(JSON.stringify(request)).not.toContain("fake_preview_value");
    expect(onNotice).toHaveBeenCalledWith("Pushed 1 variables.");
  });

  it("loads GitHub targets and creates a deployment environment", async () => {
    const user = userEvent.setup();
    const onNotice = vi.fn();
    render(
      <ProviderPushModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={onNotice}
      />,
    );

    await waitFor(() => expect(api.listGitHubRepositories).toHaveBeenCalledWith("demo-project"));
    await user.type(screen.getByLabelText("GitHub repository"), "owner/repository");
    await waitFor(() => expect(api.listGitHubEnvironments).toHaveBeenCalledWith("demo-project", "owner/repository"));
    await user.selectOptions(screen.getByLabelText(/Deployment environment/), "__new__");
    await user.type(screen.getByLabelText("New GitHub environment name"), "staging");
    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(api.createGitHubEnvironment).toHaveBeenCalledWith("demo-project", "owner/repository", "staging");
    expect(onNotice).toHaveBeenCalledWith("Created the staging GitHub Environment.");
  });

  it("auto-selects the GitHub origin for the selected env file", async () => {
    vi.mocked(api.detectGitHubRepository).mockResolvedValueOnce({ repository: "owner/repository" });
    render(
      <ProviderPushModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText("GitHub repository")).toHaveValue("owner/repository"));
    expect(api.detectGitHubRepository).toHaveBeenCalledWith("demo-project", demoProjection.files[0]?.path);
  });

  it("loads the nearest Wrangler worker and environment options", async () => {
    const user = userEvent.setup();
    render(
      <ProviderPushModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    await user.click(await screen.findByRole("button", { name: /Cloudflare Workers/ }));
    await waitFor(() => expect(screen.getByDisplayValue("demo-worker")).toBeInTheDocument());
    expect(screen.getByText("Detected from wrangler.jsonc")).toBeInTheDocument();
    expect(api.detectCloudflareTarget).toHaveBeenCalledWith("demo-project", demoProjection.files[0]?.path);
  });
});
