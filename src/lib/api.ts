import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { demoAgentIntegrations, demoProjection, demoProjects } from "./demo";
import type {
  AgentIntegrationId,
  AgentIntegrationStatus,
  AgentActivityEvent,
  CodexAccess,
  CloudflareAccessContext,
  CloudflareTargetContext,
  EasTargetContext,
  EasAccessContext,
  AwsAccessContext,
  DeploymentProviderStatus,
  GitHubEnvironmentOptions,
  GitHubRepositoryContext,
  GitHubRepositoryOptions,
  GitignoreUpdateSummary,
  ExportResult,
  ExportOccurrence,
  TeamChannel,
  TeamChannelPublishSummary,
  TeamImportPlanProjection,
  TeamImportSummary,
  TeamImportValueSide,
  MutationSummary,
  MigrationPlanProjection,
  ProjectProjection,
  ProjectSummary,
  ProviderPushRequest,
  ProviderPushResult,
  ProviderCompareRequest,
  ProviderCompareResult,
  ProviderPushReceipt,
  PersonalProviderPackInfo,
  RuntimeTarget,
  ActionPackInfo,
  ActionExecutionRequest,
  ActionExecutionResult,
} from "./types";

export const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const selectedProjectStorageKey = "env-manager.selected-project";

export class ApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (typeof error === "object" && error !== null) {
      const commandError = error as { code?: unknown; message?: unknown };
      if (typeof commandError.message === "string") {
        throw new ApiError(
          typeof commandError.code === "string" ? commandError.code : "UNKNOWN",
          commandError.message,
        );
      }
    }
    throw new ApiError("UNKNOWN", typeof error === "string" ? error : "Unknown error");
  }
}

export async function listProjects(): Promise<ProjectSummary[]> {
  if (isTauriRuntime) return call("list_projects");
  return new URLSearchParams(window.location.search).has("empty") ? [] : demoProjects;
}

export async function getLastSelectedProjectId(): Promise<string | null> {
  if (isTauriRuntime) return call("get_last_selected_project_id");
  try {
    return window.localStorage.getItem(selectedProjectStorageKey);
  } catch {
    return null;
  }
}

export async function rememberSelectedProject(projectId: string | null): Promise<void> {
  if (isTauriRuntime) {
    return call("set_last_selected_project", { request: { projectId } });
  }
  try {
    if (projectId) window.localStorage.setItem(selectedProjectStorageKey, projectId);
    else window.localStorage.removeItem(selectedProjectStorageKey);
  } catch {
    // Selection remains active for this session when browser storage is unavailable.
  }
}

export async function listAgentIntegrations(): Promise<AgentIntegrationStatus[]> {
  if (!isTauriRuntime) return demoAgentIntegrations;
  return call("list_agent_integrations");
}

export async function listDeploymentProviders(
  projectId: string,
): Promise<DeploymentProviderStatus[]> {
  if (!isTauriRuntime) {
    return [
      {
        id: "github-actions",
        name: "GitHub Actions",
        available: true,
        detail: "GitHub CLI ready",
        source: "official",
        version: null,
        targetLabel: null,
        adapter: {
          cliVersion: "2.78.0",
          profileId: "gh-secret-set-v1",
          adapterVersion: "1.0.0",
          adapterSource: "bundled",
        },
      },
      {
        id: "cloudflare-workers",
        name: "Cloudflare Workers",
        available: true,
        detail: "Compatible Wrangler ready",
        source: "official",
        version: null,
        targetLabel: null,
        adapter: {
          cliVersion: "4.115.0",
          profileId: "wrangler-secret-bulk-v1",
          adapterVersion: "1.0.0",
          adapterSource: "bundled",
        },
      },
      {
        id: "expo-eas",
        name: "Expo EAS",
        available: true,
        detail: "Compatible EAS CLI ready",
        source: "official",
        version: null,
        targetLabel: "EAS project and environments",
        adapter: {
          cliVersion: "22.2.0",
          profileId: "eas-env-set-prompt-v1",
          adapterVersion: "1.0.0",
          adapterSource: "bundled",
        },
      },
      {
        id: "aws-secrets-manager",
        name: "AWS Secrets Manager",
        available: true,
        detail: "Built-in AWS SDK",
        source: "official",
        version: "1.1.0",
        targetLabel: "Secret path prefix",
        adapter: null,
      },
      {
        id: "aws-ssm-parameter-store",
        name: "AWS SSM Parameter Store",
        available: true,
        detail: "Built-in AWS SDK · SecureString",
        source: "official",
        version: "1.1.0",
        targetLabel: "Parameter path prefix",
        adapter: null,
      },
      {
        id: "remote-runtime",
        name: "Remote Runtime",
        available: true,
        detail: "Encrypted SSH verifier",
        source: "official",
        version: "1.0.0",
        targetLabel: "Runtime target",
        adapter: null,
      },
      {
        id: "demo-hosting",
        name: "Demo Hosting CLI",
        available: true,
        detail: "Personal Provider Pack",
        source: "personal",
        version: "1.0.0",
        targetLabel: "Environment",
        adapter: {
          cliVersion: "1.4.0",
          profileId: "demo-hosting-v1",
          adapterVersion: "1.0.0",
          adapterSource: "personal",
        },
      },
    ];
  }
  return call("list_deployment_providers", { request: { projectId } });
}

export async function chooseAndInstallPersonalProviderPack(
  dialogTitle: string,
): Promise<PersonalProviderPackInfo | null> {
  if (!isTauriRuntime) return null;
  const path = await open({
    directory: false,
    multiple: false,
    title: dialogTitle,
    filters: [{ name: "Env Manager Provider Pack", extensions: ["json"] }],
  });
  if (typeof path !== "string") return null;
  return call("install_personal_provider_pack", {
    request: { path, replace: true },
  });
}

export async function removePersonalProviderPack(id: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("remove_personal_provider_pack", { request: { id } });
}

export async function listActionPacks(projectId: string): Promise<ActionPackInfo[]> {
  if (!isTauriRuntime) {
    return [{
      id: "local.demo.api-check",
      displayName: "API health check",
      description: "Call a fixed API endpoint without exposing its token.",
      packVersion: "1.0.0",
      kind: "http",
      available: true,
      bindings: [{ id: "Authorization", destination: "Authorization" }],
      target: "https://api.example.com/health",
      cliVersion: null,
      profileId: null,
    }];
  }
  return call("list_action_packs", { request: { projectId } });
}

export async function chooseAndInstallActionPack(
  dialogTitle: string,
): Promise<ActionPackInfo | null> {
  if (!isTauriRuntime) return null;
  const path = await open({
    directory: false,
    multiple: false,
    title: dialogTitle,
    filters: [{ name: "Env Manager Action Pack", extensions: ["json"] }],
  });
  if (typeof path !== "string") return null;
  return call("install_action_pack", { request: { path, replace: true } });
}

export async function removeActionPack(id: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("remove_action_pack", { request: { id } });
}

export async function executeActionPack(
  projectId: string,
  request: ActionExecutionRequest,
): Promise<ActionExecutionResult> {
  if (!isTauriRuntime) {
    return {
      packId: request.packId,
      kind: "http",
      succeeded: true,
      statusCode: 200,
      durationMs: 84,
      exitCode: null,
      resultCode: "ACTION_SUCCEEDED",
    };
  }
  return call("execute_action_pack", { payload: { projectId, request } });
}

export async function listGitHubRepositories(
  projectId: string,
): Promise<GitHubRepositoryOptions> {
  if (!isTauriRuntime) {
    return { repositories: ["owner/repository", "owner/another-project"] };
  }
  return call("list_github_repositories", { request: { projectId } });
}

export async function detectGitHubRepository(
  projectId: string,
  file: string,
): Promise<GitHubRepositoryContext> {
  if (!isTauriRuntime) return { repository: null };
  return call("detect_github_repository", { request: { projectId, file } });
}

export async function listGitHubEnvironments(
  projectId: string,
  repository: string,
): Promise<GitHubEnvironmentOptions> {
  if (!isTauriRuntime) {
    return { repository, environments: ["preview", "production"] };
  }
  return call("list_github_environments", { request: { projectId, repository } });
}

export async function createGitHubEnvironment(
  projectId: string,
  repository: string,
  environment: string,
): Promise<GitHubEnvironmentOptions> {
  if (!isTauriRuntime) {
    return { repository, environments: ["preview", "production", environment].sort() };
  }
  return call("create_github_environment", {
    request: { projectId, repository, environment },
  });
}

export async function detectCloudflareTarget(
  projectId: string,
  file: string,
): Promise<CloudflareTargetContext> {
  if (!isTauriRuntime) return { worker: null, environments: [], configPath: null, accountId: null, environmentAccountIds: {} };
  return call("detect_cloudflare_target", { request: { projectId, file } });
}

export async function detectEasTarget(
  projectId: string,
  file: string,
): Promise<EasTargetContext> {
  if (!isTauriRuntime) {
    return {
      project: "travel-pieces",
      projectId: "synthetic-project-id",
      environments: ["development", "preview", "production"],
      configPath: "apps/mobile/eas.json",
    };
  }
  return call("detect_eas_target", { request: { projectId, file } });
}

export async function inspectEasAccess(
  projectId: string,
  file: string,
  project: string | null,
): Promise<EasAccessContext> {
  if (!isTauriRuntime) {
    return {
      project: "@demo/travel-pieces",
      projectId: "synthetic-project-id",
      adapter: { cliVersion: "22.2.0", profileId: "eas-env-set-prompt-v1", adapterVersion: "1.0.0", adapterSource: "bundled" },
    };
  }
  return call("inspect_eas_access", { request: { projectId, file, project } });
}

export async function inspectCloudflareAccess(
  projectId: string,
  file: string,
  worker: string,
  environment: string | null,
): Promise<CloudflareAccessContext> {
  if (!isTauriRuntime) {
    return {
      authState: "authenticated",
      authType: "OAuth Token",
      accountState: "matched",
      accountId: "demo-account",
      accountName: "Demo account",
      accountCount: 1,
      targetState: "accessible",
      adapter: { cliVersion: "4.115.0", profileId: "wrangler-secret-bulk-v1", adapterVersion: "1.0.0", adapterSource: "bundled" },
    };
  }
  return call("inspect_cloudflare_access", {
    request: { projectId, file, worker, environment },
  });
}

export async function inspectAwsAccess(
  profile: string | null,
  region: string | null,
): Promise<AwsAccessContext> {
  if (!isTauriRuntime) {
    return {
      accountId: "123456789012",
      principalArn: "arn:aws:iam::123456789012:user/demo",
      region: region || "ap-northeast-2",
      kmsAliases: ["alias/env-manager-demo"],
      kmsAliasesAvailable: true,
    };
  }
  return call("inspect_aws_access", { request: { profile, region } });
}

export async function pushToProvider(
  projectId: string,
  request: ProviderPushRequest,
): Promise<ProviderPushResult> {
  if (!isTauriRuntime) {
    return {
      provider: request.provider,
      pushedCount: request.selections.length,
      failedKeys: [],
    };
  }
  return call("push_to_provider", { payload: { projectId, request } });
}

export async function compareProviderValues(
  projectId: string,
  request: ProviderCompareRequest,
): Promise<ProviderCompareResult> {
  if (!isTauriRuntime) {
    const runtimeTarget = request.provider === "remote-runtime"
      ? "API server · staging"
      : `${request.awsRegion ?? "ap-northeast-2"}/${request.awsPathPrefix ?? ""}`.replace(/\/$/, "");
    return {
      provider: request.provider,
      target: runtimeTarget,
      items: request.keys.map((key, index) => ({
        key,
        remoteName: request.provider === "remote-runtime"
          ? `sample-saas-staging/${key}`
          : request.awsPathPrefix ? `${request.awsPathPrefix}/${key}` : key,
        state: index === 0 ? "same" : "different",
        resultCode: null,
      })),
    };
  }
  return call("compare_provider_values", { payload: { projectId, request } });
}

export async function listRuntimeTargets(projectId: string): Promise<RuntimeTarget[]> {
  if (!isTauriRuntime) {
    return [{
      id: "demo-runtime-staging",
      displayName: "API server · staging",
      sourceFile: ".env.development",
      remoteTargetId: "sample-saas-staging",
      recipient: "age1demo00000000000000000000000000000000000000000000000000000",
      transport: { type: "ssh", destination: "deploy@example.com" },
    }];
  }
  return call("list_runtime_targets", { request: { projectId } });
}

export async function saveRuntimeTarget(
  projectId: string,
  request: RuntimeTarget,
): Promise<RuntimeTarget[]> {
  if (!isTauriRuntime) return [request];
  return call("save_runtime_target", { payload: { projectId, request } });
}

export async function removeRuntimeTarget(
  projectId: string,
  targetId: string,
): Promise<RuntimeTarget[]> {
  if (!isTauriRuntime) return [];
  return call("remove_runtime_target", { request: { projectId, targetId } });
}

export async function listProviderPushReceipts(projectId: string): Promise<ProviderPushReceipt[]> {
  if (!isTauriRuntime) return [];
  return call("list_provider_push_receipts", { request: { projectId } });
}

export async function installAgentIntegration(
  id: AgentIntegrationId,
): Promise<AgentIntegrationStatus> {
  if (!isTauriRuntime) {
    const integration = demoAgentIntegrations.find((item) => item.id === id);
    if (!integration) throw new ApiError("UNSUPPORTED_AGENT", "Unsupported AI tool");
    return {
      ...integration,
      installed: true,
      installedVersion: integration.currentVersion,
      updateAvailable: false,
      needsRepair: false,
      protection: id === "codex" ? "broker" : "guarded",
    };
  }
  return call("install_agent_integration", { request: { id } });
}

export async function chooseAndRegisterProject(dialogTitle: string): Promise<ProjectSummary | null> {
  if (!isTauriRuntime) {
    return demoProjects[0] ?? null;
  }
  const root = await open({ directory: true, multiple: false, title: dialogTitle });
  if (typeof root !== "string") {
    return null;
  }
  return call("register_project", { root });
}

export async function removeProject(projectId: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("remove_project", { request: { projectId } });
}

export async function renameProject(projectId: string, name: string): Promise<ProjectSummary> {
  if (!isTauriRuntime) {
    const project = demoProjects.find((item) => item.id === projectId) ?? {
      id: projectId,
      name,
      displayPath: "/demo/project",
    };
    return { ...project, name };
  }
  return call("rename_project", { request: { projectId, name } });
}

export async function renameEnvFile(projectId: string, file: string, name: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("rename_env_file", { request: { projectId, file, name } });
}

export async function exportEnvFiles(
  projectId: string,
  passphrase: string | null,
  selection: ExportOccurrence[] | null,
  locale: "en" | "ko",
): Promise<ExportResult> {
  if (!isTauriRuntime) return { fileCount: demoProjection.files.length, encrypted: passphrase !== null, cancelled: false };
  return call("export_env_files", { request: { projectId, passphrase, selection, locale } });
}

export async function listTeamChannels(projectId: string): Promise<TeamChannel[]> {
  if (!isTauriRuntime) {
    return [{
      id: "demo-folder-channel",
      name: "Product team · shared folder",
      readable: true,
      publishable: true,
      packages: [
        { id: "01JTEAMDEMO000000000000002", byteSize: 18_432, modifiedAtMs: Date.UTC(2026, 7, 18, 4, 15) },
        { id: "01JTEAMDEMO000000000000001", byteSize: 16_896, modifiedAtMs: Date.UTC(2026, 7, 17, 9, 30) },
      ],
    }];
  }
  return call("list_team_channels", { request: { projectId } });
}

export async function connectFolderTeamChannel(
  projectId: string,
  locale: "en" | "ko",
): Promise<TeamChannel | null> {
  if (!isTauriRuntime) {
    return {
      id: "demo-folder-channel",
      name: "Team share",
      readable: true,
      publishable: true,
      packages: [],
    };
  }
  return call("connect_folder_team_channel", { request: { projectId, locale } });
}

export async function removeTeamChannel(projectId: string, channelId: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("remove_team_channel", { request: { projectId, channelId } });
}

export async function publishTeamChannel(
  projectId: string,
  channelId: string,
  passphrase: string,
  selection: ExportOccurrence[] | null,
): Promise<TeamChannelPublishSummary> {
  if (!isTauriRuntime) return { packageId: "demo-package", fileCount: demoProjection.files.length };
  return call("publish_team_channel", {
    request: { projectId, channelId, passphrase, selection },
  });
}

export async function planTeamChannelImport(
  projectId: string,
  channelId: string,
  packageId: string,
  passphrase: string,
): Promise<TeamImportPlanProjection> {
  if (!isTauriRuntime) {
    const plan = await planTeamImport(projectId, passphrase, "en");
    if (!plan) throw new ApiError("UNAVAILABLE", "Demo import plan is unavailable");
    return plan;
  }
  return call("plan_team_channel_import", {
    request: { projectId, channelId, packageId, passphrase },
  });
}

export async function planTeamImport(
  projectId: string,
  passphrase: string,
  locale: "en" | "ko",
): Promise<TeamImportPlanProjection | null> {
  if (!isTauriRuntime) {
    return {
      planId: "demo-team-import-plan",
      expiresInSeconds: 300,
      preview: {
        files: [
          {
            path: ".env.local",
            targetPath: ".env.local",
            occurrences: [
              { id: "demo-gpt-local", key: "GPT_API_KEY", state: "conflict", linkId: "demo-gpt-link" },
              { id: "demo-base-url", key: "OPENAI_BASE_URL", state: "new", linkId: null },
            ],
          },
          {
            path: ".env.development",
            targetPath: ".env.development",
            occurrences: [
              { id: "demo-gpt-development", key: "GPT_API_KEY", state: "conflict", linkId: "demo-gpt-link" },
            ],
          },
          {
            path: "apps/web/.env.local",
            targetPath: "apps/web/.env.local",
            occurrences: [
              { id: "demo-public-url", key: "VITE_API_BASE_URL", state: "conflict", linkId: null },
            ],
          },
        ],
        newCount: 1,
        unchangedCount: 0,
        conflictCount: 3,
      },
    };
  }
  return call("plan_team_import", { request: { projectId, passphrase, locale } });
}

export async function applyTeamImport(
  projectId: string,
  planId: string,
  sharedConflicts: string[],
): Promise<TeamImportSummary> {
  if (!isTauriRuntime) {
    return {
      addedCount: 1,
      updatedCount: sharedConflicts.length,
      unchangedCount: 0,
      keptLocalCount: 3 - sharedConflicts.length,
      affectedFiles: [".env.local"],
    };
  }
  return call("apply_team_import", { request: { projectId, planId, sharedConflicts } });
}

export async function remapTeamImportFile(
  projectId: string,
  planId: string,
  sourceFile: string,
  targetFile: string,
): Promise<TeamImportPlanProjection["preview"]> {
  if (!isTauriRuntime) {
    const demoPlan = await planTeamImport(projectId, "fake-demo-passphrase", "en");
    if (!demoPlan) throw new ApiError("UNAVAILABLE", "Demo import plan is unavailable");
    const files = demoPlan.preview.files.map((file) => file.path === sourceFile
      ? {
          ...file,
          targetPath: targetFile,
          occurrences: file.occurrences.map((occurrence) => ({ ...occurrence, state: "new" as const, linkId: null })),
        }
      : file);
    const occurrences = files.flatMap((file) => file.occurrences);
    return {
      files,
      newCount: occurrences.filter((item) => item.state === "new").length,
      unchangedCount: occurrences.filter((item) => item.state === "unchanged").length,
      conflictCount: occurrences.filter((item) => item.state === "conflict").length,
    };
  }
  return call("remap_team_import_file", {
    request: { projectId, planId, sourceFile, targetFile },
  });
}

export async function revealTeamImportConflict(
  projectId: string,
  planId: string,
  occurrenceId: string,
  side: TeamImportValueSide,
): Promise<string> {
  if (!isTauriRuntime) return side === "local" ? "fake_local_value" : "fake_shared_value";
  return call("reveal_team_import_conflict", {
    request: { projectId, planId, occurrenceId, side },
  });
}

export async function discardTeamImport(projectId: string, planId: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("discard_team_import", { request: { projectId, planId } });
}

export async function scanProject(projectId: string): Promise<ProjectProjection> {
  if (!isTauriRuntime) return demoProjection;
  return call("scan_project", { request: { projectId } });
}

export async function applyGitignoreGuard(projectId: string): Promise<GitignoreUpdateSummary> {
  if (!isTauriRuntime) {
    return {
      addedPatterns: demoProjection.gitSafety.missingIgnoreFiles.map((path) => `/${path}`),
      trackedFiles: demoProjection.gitSafety.trackedFiles,
    };
  }
  return call("apply_gitignore_guard", { request: { projectId } });
}

export async function saveValue(
  projectId: string,
  request: { file: string; key: string; newValue: string },
): Promise<MutationSummary> {
  if (!isTauriRuntime) {
    return { affectedFiles: [request.file], keys: [request.key] };
  }
  return call("save_value", { payload: { projectId, request } });
}

export async function saveDescription(
  projectId: string,
  request: { file: string; key: string; lines: string[] },
): Promise<MutationSummary> {
  if (!isTauriRuntime) {
    return { affectedFiles: [request.file], keys: [request.key] };
  }
  return call("save_description", { payload: { projectId, request } });
}

export async function createGroup(
  projectId: string,
  request: { file: string; name: string },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [request.file], keys: [] };
  return call("create_group", { payload: { projectId, request } });
}

export async function renameGroup(
  projectId: string,
  request: { file: string; currentName: string; newName: string },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [request.file], keys: [] };
  return call("rename_group", { payload: { projectId, request } });
}

export async function addVariable(
  projectId: string,
  request: {
    file: string;
    key: string;
    group: string;
    description: string[];
    value: string;
  },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [request.file], keys: [request.key] };
  return call("add_variable", { payload: { projectId, request } });
}

export async function deleteVariable(
  projectId: string,
  request: { file: string; key: string },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [request.file], keys: [request.key] };
  return call("delete_variable", {
    payload: { projectId, request },
    confirmed: true,
  });
}

export async function moveVariable(
  projectId: string,
  request: { file: string; key: string; targetGroup: string },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [request.file], keys: [request.key] };
  return call("move_variable", { payload: { projectId, request } });
}

export async function createLink(
  projectId: string,
  request: { key: string; files: string[]; sourceFile: string | null },
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: request.files, keys: [request.key] };
  return call("create_link", { payload: { projectId, request } });
}

export async function detachLink(
  projectId: string,
  linkId: string,
  file: string,
): Promise<void> {
  if (!isTauriRuntime) return;
  return call("detach_link_member", { request: { projectId, linkId, file } });
}

export async function setCodexAccess(
  projectId: string,
  key: string,
  access: CodexAccess,
  confirmed: boolean,
): Promise<void> {
  if (!isTauriRuntime) return;
  return call("set_codex_access", { request: { projectId, key, access, confirmed } });
}

export async function protectVariables(projectId: string, keys: string[]): Promise<void> {
  if (!isTauriRuntime) return;
  return call("protect_variables", { request: { projectId, keys } });
}

export async function listAgentActivity(projectId: string): Promise<AgentActivityEvent[]> {
  if (!isTauriRuntime) {
    return [
      {
        timestampMs: Date.now() - 45_000,
        projectId,
        actor: "codex",
        category: "structure-inspection",
        operation: "inspect_project",
        relativePaths: [],
        variableNames: [],
        policyDecision: "redacted",
        outcome: "allowed",
        resultCode: "OK",
      },
      {
        timestampMs: Date.now() - 180_000,
        projectId,
        actor: "claude-code",
        category: "value-read",
        operation: "read_allowed_value",
        relativePaths: [".env.local"],
        variableNames: ["GPT_API_KEY"],
        policyDecision: "policy-checked",
        outcome: "blocked",
        resultCode: "CODEX_ACCESS_BLOCKED",
      },
    ];
  }
  return call("list_agent_activity", { request: { projectId } });
}

export async function readValue(
  projectId: string,
  file: string,
  key: string,
): Promise<string> {
  if (!isTauriRuntime) return "fake_preview_value";
  return call("read_value", { request: { projectId, file, key } });
}

export async function copyValue(projectId: string, file: string, key: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("copy_value", { request: { projectId, file, key } });
}

export async function copyKey(projectId: string, key: string): Promise<void> {
  if (!isTauriRuntime) return;
  return call("copy_key", { request: { projectId, key } });
}

export async function planMigration(
  projectId: string,
  file: string,
): Promise<MigrationPlanProjection> {
  if (!isTauriRuntime) {
    return {
      planId: "demo-migration-plan",
      expiresInSeconds: 300,
      preview: {
        file,
        summary:
          "Convert 2 group markers to the `# @group` format without changing values or variable order.",
        suggestions: [
          { currentMarker: "# === GPT ===", groupName: "GPT" },
          { currentMarker: "# [Database]", groupName: "Database" },
        ],
      },
    };
  }
  return call("plan_migration", { request: { projectId, file } });
}

export async function applyMigration(
  projectId: string,
  planId: string,
): Promise<MutationSummary> {
  if (!isTauriRuntime) return { affectedFiles: [], keys: [] };
  return call("apply_migration", {
    request: { projectId, planId, confirmed: true },
  });
}
