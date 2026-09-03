import type { AwsAccessContext, CloudflareAccessContext, CloudflareTargetContext, EasAccessContext, EasTargetContext, GitHubEnvironmentOptions, GitHubRepositoryContext, GitHubRepositoryOptions, ProviderCompareRequest, ProviderCompareResult, ProviderPushReceipt, ProviderPushRequest, ProviderPushResult, RuntimeTarget } from "../types";
import { call, isTauriRuntime } from "./shared";

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
      kmsAliases: ["alias/kavranta-demo"],
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
