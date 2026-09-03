import { open } from "@tauri-apps/plugin-dialog";

import { demoAgentIntegrations } from "../demo";
import type { ActionExecutionRequest, ActionExecutionResult, ActionPackInfo, AgentIntegrationId, AgentIntegrationStatus, DeploymentProviderStatus, PersonalProviderPackInfo } from "../types";
import { ApiError, call, isTauriRuntime } from "./shared";

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
    filters: [{ name: "Kavranta Provider Pack", extensions: ["json"] }],
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
    filters: [{ name: "Kavranta Action Pack", extensions: ["json"] }],
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
