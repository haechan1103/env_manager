import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ProviderPushModal } from "./ProviderPushModal";

vi.mock("../../lib/api", () => ({
  listDeploymentProviders: vi.fn(async () => [
    { id: "github-actions", name: "GitHub Actions", available: true, detail: "ready", source: "official", version: null, targetLabel: null, adapter: { cliVersion: "2.78.0", profileId: "gh-secret-set-v1", adapterVersion: "1.0.0", adapterSource: "bundled" } },
    { id: "cloudflare-workers", name: "Cloudflare Workers", available: true, detail: "ready", source: "official", version: null, targetLabel: null, adapter: { cliVersion: "4.115.0", profileId: "wrangler-secret-bulk-v1", adapterVersion: "1.0.0", adapterSource: "bundled" } },
    { id: "aws-secrets-manager", name: "AWS Secrets Manager", available: true, detail: "ready", source: "official", version: "1.0.0", targetLabel: "Secret path prefix", adapter: null },
    { id: "aws-ssm-parameter-store", name: "AWS SSM Parameter Store", available: true, detail: "ready", source: "official", version: "1.0.0", targetLabel: "Parameter path prefix", adapter: null },
    { id: "remote-runtime", name: "Remote Runtime", available: true, detail: "ready", source: "official", version: "1.0.0", targetLabel: "Runtime target", adapter: null },
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
    accountId: "demo-account",
    environmentAccountIds: {},
  })),
  inspectCloudflareAccess: vi.fn(async () => ({
    authState: "authenticated",
    authType: "OAuth Token",
    accountState: "matched",
    accountId: "demo-account",
    accountName: "Demo account",
    accountCount: 1,
    targetState: "accessible",
    adapter: { cliVersion: "4.115.0", profileId: "wrangler-secret-bulk-v1", adapterVersion: "1.0.0", adapterSource: "bundled" },
  })),
  inspectAwsAccess: vi.fn(async (_profile: string | null, region: string | null) => ({
    accountId: "123456789012",
    principalArn: "arn:aws:iam::123456789012:user/demo",
    region: region || "ap-northeast-2",
    kmsAliases: ["alias/kavranta-demo"],
    kmsAliasesAvailable: true,
  })),
  chooseAndInstallPersonalProviderPack: vi.fn(async () => null),
  removePersonalProviderPack: vi.fn(async () => undefined),
  listProviderPushReceipts: vi.fn(async () => []),
  listRuntimeTargets: vi.fn(async () => [{
    id: "mobile-ok-dev",
    displayName: "mobile-ok · dev",
    sourceFile: ".env.local",
    remoteTargetId: "mobile-ok-dev",
    recipient: "age1fakepublicrecipient",
    transport: { type: "ssh" as const, destination: "deploy@example.test" },
  }]),
  saveRuntimeTarget: vi.fn(async (_projectId: string, request: unknown) => [request]),
  removeRuntimeTarget: vi.fn(async () => []),
  compareProviderValues: vi.fn(async (_projectId: string, request: { provider: string; keys: string[]; awsPathPrefix: string | null }) => ({
    provider: request.provider,
    target: "ap-northeast-2/demo/staging",
    items: request.keys.map((key) => ({
      key,
      remoteName: request.awsPathPrefix ? `${request.awsPathPrefix}/${key}` : key,
      state: "same" as const,
      resultCode: null,
    })),
  })),
  pushToProvider: vi.fn(async (_projectId: string, request: { provider: string; selections: unknown[] }) => ({
    provider: request.provider,
    pushedCount: request.selections.length,
    failedKeys: [],
  })),
}));

describe("ProviderPushModal", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders the modal shell while CLI discovery is still pending", async () => {
    vi.mocked(api.listDeploymentProviders).mockReturnValueOnce(
      new Promise<Awaited<ReturnType<typeof api.listDeploymentProviders>>>(() => undefined),
    );
    render(
      <ProviderPushModal
        projectId="demo-project"
        projection={demoProjection}
        onClose={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Push variables" })).toBeInTheDocument();
    expect(screen.getAllByText("Checking CLI…")).toHaveLength(2);
    expect(screen.getByText("Variables to push")).toBeInTheDocument();
    await waitFor(() => expect(api.listDeploymentProviders).toHaveBeenCalledWith("demo-project"));
  });

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
    await waitFor(() => expect(screen.getAllByText("CLI ready")).toHaveLength(4));
    expect(api.detectCloudflareTarget).not.toHaveBeenCalled();
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

  it("lets a GitHub kind be chosen before selecting the variable", async () => {
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

    const kind = (await screen.findAllByRole("combobox", { name: /Remote type for/ }))[0];
    expect(kind).toBeEnabled();
    await user.selectOptions(kind!, "variable");
    expect(screen.getByText(/GitHub Variables are not secret/)).toBeInTheDocument();
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    await user.click(checkboxes[0]!);
    await user.type(screen.getByLabelText("GitHub repository"), "owner/repository");
    await user.click(screen.getByRole("button", { name: "Push 1" }));

    expect(api.pushToProvider).toHaveBeenLastCalledWith(
      "demo-project",
      expect.objectContaining({ selections: [expect.objectContaining({ kind: "variable" })] }),
    );
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
    expect(screen.getAllByText("Worker Secret").length).toBeGreaterThan(0);
    expect(api.detectCloudflareTarget).toHaveBeenCalledWith("demo-project", demoProjection.files[0]?.path);
    await waitFor(() => expect(api.inspectCloudflareAccess).toHaveBeenCalledWith(
      "demo-project",
      demoProjection.files[0]?.path,
      "demo-worker",
      null,
    ));
    expect(screen.getByText("Cloudflare target verified")).toBeInTheDocument();
  });

  it("explains a missing Wrangler login before any value can be pushed", async () => {
    vi.mocked(api.inspectCloudflareAccess).mockResolvedValueOnce({
      authState: "not-authenticated",
      authType: null,
      accountState: "unchecked",
      accountId: "demo-account",
      accountName: null,
      accountCount: 0,
      targetState: "unchecked",
      adapter: { cliVersion: "4.115.0", profileId: "wrangler-secret-bulk-v1", adapterVersion: "1.0.0", adapterSource: "bundled" },
    });
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
    expect(await screen.findByText(/Wrangler is not signed in/)).toBeInTheDocument();
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    await user.click(checkboxes[0]!);
    expect(screen.getByRole("button", { name: "Push 1" })).toBeDisabled();
  });

  it("preflights AWS and sends only names with profile, Region, prefix, and KMS metadata", async () => {
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

    await user.click(await screen.findByRole("button", { name: /AWS Secrets Manager/ }));
    await waitFor(() => expect(api.inspectAwsAccess).toHaveBeenCalled());
    expect(await screen.findByText("AWS account verified", {}, { timeout: 3_000 })).toBeInTheDocument();
    await user.type(screen.getByLabelText(/AWS profile/), "staging");
    await user.clear(screen.getByLabelText(/AWS Region/));
    await user.type(screen.getByLabelText(/AWS Region/), "ap-northeast-2");
    await user.type(screen.getByLabelText(/Secret path prefix/), "demo/staging");
    await user.type(screen.getByLabelText(/KMS key or alias/), "alias/kavranta-demo");
    await waitFor(() => expect(screen.getByText("AWS account verified")).toBeInTheDocument());
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    await user.click(checkboxes[0]!);
    await user.click(screen.getByRole("button", { name: "Push 1" }));

    expect(api.pushToProvider).toHaveBeenLastCalledWith(
      "demo-project",
      expect.objectContaining({
        provider: "aws-secrets-manager",
        awsProfile: "staging",
        awsRegion: "ap-northeast-2",
        awsPathPrefix: "demo/staging",
        awsKmsKeyId: "alias/kavranta-demo",
        selections: [expect.objectContaining({ kind: "secret" })],
      }),
    );
    expect(JSON.stringify(vi.mocked(api.pushToProvider).mock.calls.at(-1))).not.toContain("fake_preview_value");
  });

  it("compares AWS values without putting a candidate value in the request", async () => {
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

    await user.click(await screen.findByRole("button", { name: /AWS Secrets Manager/ }));
    expect(await screen.findByText("AWS account verified", {}, { timeout: 3_000 })).toBeInTheDocument();
    await user.type(screen.getByLabelText(/Secret path prefix/), "demo/staging");
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    await user.click(checkboxes[0]!);
    await user.click(screen.getByRole("button", { name: "Check 1" }));

    expect(await screen.findByText("Current deployment value check")).toBeInTheDocument();
    expect(screen.getByText("Same")).toBeInTheDocument();
    const request = vi.mocked(api.compareProviderValues).mock.calls.at(-1);
    expect(request?.[1]).toEqual(expect.objectContaining({
      provider: "aws-secrets-manager",
      keys: [expect.any(String)],
      awsPathPrefix: "demo/staging",
    }));
    expect(JSON.stringify(request)).not.toContain("fake_preview_value");
  });

  it("compares a registered Runtime target without sending a path or candidate through React", async () => {
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

    await user.click(await screen.findByRole("button", { name: /Remote Runtime/ }));
    await waitFor(() => expect(api.listRuntimeTargets).toHaveBeenCalledWith("demo-project"));
    expect(screen.getByText("mobile-ok · dev · SSH")).toBeInTheDocument();
    const checkboxes = screen.getAllByRole("checkbox").filter((item) => !item.hasAttribute("disabled"));
    await user.click(checkboxes[0]!);
    await user.click(screen.getByRole("button", { name: "Check 1" }));

    const request = vi.mocked(api.compareProviderValues).mock.calls.at(-1);
    expect(request?.[1]).toEqual(expect.objectContaining({
      provider: "remote-runtime",
      runtimeTargetId: "mobile-ok-dev",
      file: ".env.local",
      keys: [expect.any(String)],
    }));
    expect(JSON.stringify(request)).not.toContain("/srv/");
    expect(JSON.stringify(request)).not.toContain("fake_preview_value");
    expect(screen.queryByRole("button", { name: /Push 1/ })).not.toBeInTheDocument();
  });

  it("removes a locally installed Personal Provider Pack without touching env values", async () => {
    vi.mocked(api.listDeploymentProviders)
      .mockResolvedValueOnce([
        { id: "github-actions", name: "GitHub Actions", available: true, detail: "ready", source: "official", version: null, targetLabel: null, adapter: null },
        { id: "local.acme.deploy", name: "Acme Deploy", available: true, detail: "ready", source: "personal", version: "1.0.0", targetLabel: "Service", adapter: null },
      ])
      .mockResolvedValueOnce([
        { id: "github-actions", name: "GitHub Actions", available: true, detail: "ready", source: "official", version: null, targetLabel: null, adapter: null },
      ]);
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

    await user.click(await screen.findByRole("button", { name: /Acme Deploy/ }));
    await user.click(screen.getByRole("button", { name: "Remove Pack" }));

    await waitFor(() => expect(api.removePersonalProviderPack).toHaveBeenCalledWith("local.acme.deploy"));
    expect(onNotice).toHaveBeenCalledWith("Removed Acme Deploy from this computer.");
    expect(api.pushToProvider).not.toHaveBeenCalled();
  });
});
