import { demoProjection } from "../demo";
import type { ExportOccurrence, ExportResult, TeamChannel, TeamChannelPublishSummary, TeamImportPlanProjection, TeamImportSummary, TeamImportValueSide } from "../types";
import { ApiError, call, isTauriRuntime } from "./shared";

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
