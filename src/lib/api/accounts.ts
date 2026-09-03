import { demoProjects } from "../demo";
import type { AccountField, AccountProjection, CreateAccountRequest, ProjectSummary, UpdateAccountRequest } from "../types";
import { call, isTauriRuntime, selectedProjectStorageKey } from "./shared";

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

export async function listAccounts(projectId: string): Promise<AccountProjection[]> {
  if (!isTauriRuntime) return [];
  return call("list_accounts", { request: { projectId } });
}

export async function createAccount(
  projectId: string,
  request: CreateAccountRequest,
): Promise<AccountProjection> {
  return call("create_account", { request: { projectId, ...request } });
}

export async function updateAccount(
  projectId: string,
  request: UpdateAccountRequest,
): Promise<void> {
  return call("update_account", { request: { projectId, ...request } });
}

export async function deleteAccount(projectId: string, accountId: string): Promise<void> {
  return call("delete_account", { request: { projectId, accountId } });
}

export async function setAccountProjectAccess(
  projectId: string,
  accountId: string,
  allowed: boolean,
): Promise<void> {
  return call("set_account_project_access", { request: { projectId, accountId, allowed } });
}

export async function copyAccountField(
  projectId: string,
  accountId: string,
  field: AccountField,
): Promise<void> {
  return call("copy_account_field", { request: { projectId, accountId, field } });
}
