import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

import { isTauriRuntime } from "../../lib/api";
import { APP_VERSION } from "../../lib/version";

export interface AppUpdateInfo {
  currentVersion: string;
  version: string;
  notes: string | null;
}

let pendingUpdate: Update | null = null;

export async function currentAppVersion(): Promise<string> {
  return isTauriRuntime ? getVersion() : APP_VERSION;
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  if (!isTauriRuntime) return null;

  const previousUpdate = pendingUpdate;
  const update = await check({ timeout: 10_000 });
  if (!update) {
    if (previousUpdate) await previousUpdate.close();
    pendingUpdate = null;
    return null;
  }

  pendingUpdate = update;
  if (previousUpdate && previousUpdate !== update) {
    await previousUpdate.close();
  }
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
