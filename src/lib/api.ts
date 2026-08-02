import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { demoProjection, demoProjects } from "./demo";
import type {
  CodexAccess,
  MutationSummary,
  MigrationPlanProjection,
  ProjectProjection,
  ProjectSummary,
} from "./types";

export const isTauriRuntime = "__TAURI_INTERNALS__" in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (typeof error === "object" && error !== null && "message" in error) {
      throw new Error(String(error.message));
    }
    throw new Error(typeof error === "string" ? error : "알 수 없는 오류가 발생했습니다.");
  }
}

export async function listProjects(): Promise<ProjectSummary[]> {
  if (isTauriRuntime) return call("list_projects");
  return new URLSearchParams(window.location.search).has("empty") ? [] : demoProjects;
}

export async function chooseAndRegisterProject(): Promise<ProjectSummary | null> {
  if (!isTauriRuntime) {
    return demoProjects[0] ?? null;
  }
  const root = await open({ directory: true, multiple: false, title: "프로젝트 폴더 선택" });
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
          "2개 그룹 표식을 `# @group` 형식으로 바꿉니다. 변수 값과 순서는 바꾸지 않습니다.",
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
