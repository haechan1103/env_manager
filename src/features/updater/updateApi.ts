import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

import { isTauriRuntime } from "../../lib/api";

export interface AppUpdateInfo {
  currentVersion: string;
  version: string;
  notes: string | null;
}

let pendingUpdate: Update | null = null;

export async function currentAppVersion(): Promise<string> {
  return isTauriRuntime ? getVersion() : "0.3.0";
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  if (!isTauriRuntime) return null;

  if (pendingUpdate) {
    await pendingUpdate.close();
    pendingUpdate = null;
  }

  const update = await check({ timeout: 10_000 });
  if (!update) {
    pendingUpdate = null;
    return null;
  }

  pendingUpdate = update;
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    notes: update.body?.trim() || null,
  };
}

export async function installAppUpdate(): Promise<void> {
  if (!pendingUpdate) {
    throw new Error("설치할 업데이트가 없습니다. 다시 확인해주세요.");
  }
  await pendingUpdate.downloadAndInstall();
  await relaunch();
}
