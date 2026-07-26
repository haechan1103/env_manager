import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useMemo, useState } from "react";

import * as api from "../lib/api";
import type { ProjectProjection, ProjectSummary } from "../lib/types";

export function useEnvManager() {
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
      setError(cause instanceof Error ? cause.message : "프로젝트를 불러오지 못했습니다.");
    } finally {
      setLoading(false);
    }
  }, []);

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
          setError(cause instanceof Error ? cause.message : "변경 파일을 다시 읽지 못했습니다.");
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
  }, [refreshProject]);

  const register = useCallback(async () => {
    setError(null);
    try {
      const project = await api.chooseAndRegisterProject();
      if (!project) return;
      setProjects((current) => {
        const remaining = current.filter((item) => item.id !== project.id);
        return [...remaining, project].sort((left, right) => left.name.localeCompare(right.name));
      });
      setSelectedProjectId(project.id);
      await refreshProject(project.id);
      setNotice(`${project.name} 프로젝트를 등록했습니다.`);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "프로젝트를 등록하지 못했습니다.");
    }
  }, [refreshProject]);

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
        setNotice("프로젝트 등록만 제거했습니다. 프로젝트 파일은 변경하지 않았습니다.");
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : "프로젝트 등록을 제거하지 못했습니다.");
      }
    },
    [projects],
  );

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
    refreshProject,
    clearError,
    clearNotice,
    showError: setError,
    showNotice: setNotice,
  };
}
