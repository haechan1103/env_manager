import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";

import * as api from "../lib/api";
import { localizeError, useI18n } from "../i18n";
import type { ProjectProjection, ProjectSummary } from "../lib/types";

export function useEnvManager() {
  const { locale, t } = useI18n();
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [projections, setProjections] = useState<Record<string, ProjectProjection>>({});
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refreshProject = useCallback(async (projectId: string) => {
    const projection = await api.scanProject(projectId);
    setProjections((current) => ({ ...current, [projectId]: projection }));
    return projection;
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await api.listProjects();
      setProjects(items);
      setSelectedProjectId((current) => current ?? items[0]?.id ?? null);
      const scans = await Promise.all(
        items.map(async (project) => [project.id, await api.scanProject(project.id)] as const),
      );
      setProjections(Object.fromEntries(scans));
    } catch (cause) {
      setError(localizeError(cause, locale, "error.loadProjects"));
    } finally {
      setLoading(false);
    }
  }, [locale]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!api.isTauriRuntime) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<{ projectId: string }>("managed-files-changed", (event) => {
      if (!cancelled) {
        void refreshProject(event.payload.projectId).catch((cause: unknown) => {
          setError(localizeError(cause, locale, "error.rescan"));
        });
      }
    }).then((cleanup) => {
      if (cancelled) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [locale, refreshProject]);

  const register = useCallback(async () => {
    setError(null);
    try {
      const project = await api.chooseAndRegisterProject(t("app.chooseFolderTitle"));
      if (!project) return;
      setProjects((current) => {
        const remaining = current.filter((item) => item.id !== project.id);
        return [...remaining, project].sort((left, right) => left.name.localeCompare(right.name));
      });
      setSelectedProjectId(project.id);
      await refreshProject(project.id);
      setNotice(t("notice.projectRegistered", { name: project.name }));
    } catch (cause) {
      setError(localizeError(cause, locale, "error.register"));
    }
  }, [locale, refreshProject, t]);

  const remove = useCallback(
    async (projectId: string) => {
      try {
        await api.removeProject(projectId);
        setProjects((current) => current.filter((project) => project.id !== projectId));
        setProjections((current) => {
          const next = { ...current };
          delete next[projectId];
          return next;
        });
        setSelectedProjectId((current) => {
          if (current !== projectId) return current;
          return projects.find((project) => project.id !== projectId)?.id ?? null;
        });
        setNotice(t("notice.projectRemoved"));
      } catch (cause) {
        setError(localizeError(cause, locale, "error.remove"));
      }
    },
    [locale, projects, t],
  );

  const renameProject = useCallback(async (projectId: string, name: string) => {
    try {
      const updated = await api.renameProject(projectId, name);
      setProjects((current) => current
        .map((project) => project.id === projectId ? updated : project)
        .sort((left, right) => left.name.localeCompare(right.name)));
      setNotice(t("notice.projectRenamed", { name: updated.name }));
    } catch (cause) {
      setError(localizeError(cause, locale, "error.rename"));
    }
  }, [locale, t]);

  const renameEnvFile = useCallback(async (projectId: string, file: string, name: string) => {
    try {
      await api.renameEnvFile(projectId, file, name);
      await refreshProject(projectId);
      setNotice(t("notice.fileRenamed", { name }));
    } catch (cause) {
      setError(localizeError(cause, locale, "error.rename"));
    }
  }, [locale, refreshProject, t]);

  const applyGitignoreGuard = useCallback(async () => {
    if (!selectedProjectId) return;
    setError(null);
    try {
      const summary = await api.applyGitignoreGuard(selectedProjectId);
      await refreshProject(selectedProjectId);
      setNotice(
        summary.trackedFiles.length > 0
          ? t("notice.gitignoreAddedTracked", {
              patterns: summary.addedPatterns.length,
              tracked: summary.trackedFiles.length,
            })
          : t("notice.gitignoreAdded", { count: summary.addedPatterns.length }),
      );
    } catch (cause) {
      setError(localizeError(cause, locale, "error.gitSafety"));
      throw cause;
    }
  }, [locale, refreshProject, selectedProjectId, t]);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedProjectId) ?? null,
    [projects, selectedProjectId],
  );
  const projection = selectedProjectId ? projections[selectedProjectId] ?? null : null;
  const clearError = useCallback(() => setError(null), []);
  const clearNotice = useCallback(() => setNotice(null), []);

  return {
    projects,
    selectedProject,
    selectedProjectId,
    projection,
    loading,
    error,
    notice,
    selectProject: setSelectedProjectId,
    register,
    remove,
    renameProject,
    renameEnvFile,
    applyGitignoreGuard,
    refreshProject,
    clearError,
    clearNotice,
    showError: setError,
    showNotice: setNotice,
  };
}
