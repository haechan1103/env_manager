import { invoke } from "@tauri-apps/api/core";

export const isTauriRuntime = "__TAURI_INTERNALS__" in window;
export const selectedProjectStorageKey = "env-manager.selected-project";

export class ApiError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
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
