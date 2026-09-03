import { useEffect, useState } from "react";

import * as api from "../../../lib/api";
import type { EasAccessContext, EasTargetContext } from "../../../lib/types";

interface Options {
  projectId: string;
  file: string;
  active: boolean;
}

export function useEasTarget({ projectId, file, active }: Options) {
  const [easTarget, setEasTarget] = useState<EasTargetContext | null>(null);
  const [easProject, setEasProject] = useState("");
  const [easEnvironments, setEasEnvironments] = useState<string[]>([]);
  const [loadingEasTarget, setLoadingEasTarget] = useState(false);
  const [easAccess, setEasAccess] = useState<EasAccessContext | null>(null);
  const [easAccessError, setEasAccessError] = useState(false);

  useEffect(() => {
    if (!active) return;
    let current = true;
    setLoadingEasTarget(true);
    setEasTarget(null);
    setEasProject("");
    setEasEnvironments([]);
    setEasAccess(null);
    setEasAccessError(false);
    void api.detectEasTarget(projectId, file)
      .then(async (target) => {
        if (!current) return;
        setEasTarget(target);
        const detectedProject = target.project ?? target.projectId ?? "";
        setEasProject(detectedProject);
        setEasEnvironments(target.environments);
        const access = await api.inspectEasAccess(projectId, file, detectedProject || null);
        if (current) setEasAccess(access);
      })
      .catch(() => {
        if (!current) return;
        setEasAccess(null);
        setEasAccessError(true);
      })
      .finally(() => {
        if (current) setLoadingEasTarget(false);
      });
    return () => { current = false; };
  }, [active, file, projectId]);

  return {
    easTarget,
    easProject,
    easEnvironments,
    setEasEnvironments,
    loadingEasTarget,
    easAccess,
    easAccessError,
  };
}
