import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { demoAgentIntegrations, demoProjection, demoProjects } from "./demo";
import type {
  AgentIntegrationId,
  AgentIntegrationStatus,
  CodexAccess,
  MutationSummary,
  MigrationPlanProjection,
  ProjectProjection,
  ProjectSummary,
} from "./types";

export const isTauriRuntime = "__TAURI_INTERNALS__" in window;

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

export async function listAgentIntegrations(): Promise<AgentIntegrationStatus[]> {
  if (!isTauriRuntime) return demoAgentIntegrations;
  return call("list_agent_integrations");
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

export async function scanProject(projectId: string): Promise<ProjectProjection> {
  if (!isTauriRuntime) return demoProjection;
  return call("scan_project", { request: { projectId } });
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
