import { useEffect, useState } from "react";

import { localizeError, useI18n } from "../../../i18n";
import * as api from "../../../lib/api";

interface Options {
  projectId: string;
  file: string;
  active: boolean;
  available: boolean;
  onNotice: (message: string) => void;
}

export function useGitHubTarget({ projectId, file, active, available, onNotice }: Options) {
  const { locale, t } = useI18n();
  const [repository, setRepository] = useState("");
  const [githubEnvironment, setGithubEnvironment] = useState("");
  const [githubRepositories, setGithubRepositories] = useState<string[]>([]);
  const [githubEnvironments, setGithubEnvironments] = useState<string[]>([]);
  const [newGithubEnvironment, setNewGithubEnvironment] = useState("");
  const [loadingRepositories, setLoadingRepositories] = useState(false);
  const [loadingEnvironments, setLoadingEnvironments] = useState(false);
  const [creatingEnvironment, setCreatingEnvironment] = useState(false);
  const [githubTargetError, setGithubTargetError] = useState<string | null>(null);
  const [detectingRepository, setDetectingRepository] = useState(true);

  useEffect(() => {
    if (!active) return;
    let current = true;
    setDetectingRepository(true);
    void api.detectGitHubRepository(projectId, file)
      .then((result) => {
        if (current && result.repository) setRepository(result.repository);
      })
      .catch(() => undefined)
      .finally(() => {
        if (current) setDetectingRepository(false);
      });
    return () => { current = false; };
  }, [active, file, projectId]);

  useEffect(() => {
    if (!active || !available) return;
    let current = true;
    setLoadingRepositories(true);
    void api.listGitHubRepositories(projectId)
      .then((result) => {
        if (!current) return;
        setGithubRepositories(result.repositories);
        setGithubTargetError(null);
      })
      .catch((error) => {
        if (current) setGithubTargetError(localizeError(error, locale, "push.targetsError"));
      })
      .finally(() => {
        if (current) setLoadingRepositories(false);
      });
    return () => { current = false; };
  }, [active, available, locale, projectId]);

  const targetValid = /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(repository);

  useEffect(() => {
    if (!active) return;
    const normalizedRepository = repository.trim();
    if (!/^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(normalizedRepository)) {
      setGithubEnvironments([]);
      setGithubEnvironment("");
      setLoadingEnvironments(false);
      return;
    }
    let current = true;
    setLoadingEnvironments(true);
    const timeout = window.setTimeout(() => {
      void api.listGitHubEnvironments(projectId, normalizedRepository)
        .then((result) => {
          if (!current || result.repository !== normalizedRepository) return;
          setGithubEnvironments(result.environments);
          setGithubEnvironment((environment) => (
            environment === "__new__" || result.environments.includes(environment)
              ? environment
              : ""
          ));
          setGithubTargetError(null);
        })
        .catch((error) => {
          if (!current) return;
          setGithubEnvironments([]);
          setGithubTargetError(localizeError(error, locale, "push.targetsError"));
        })
        .finally(() => {
          if (current) setLoadingEnvironments(false);
        });
    }, 250);
    return () => {
      current = false;
      window.clearTimeout(timeout);
    };
  }, [active, locale, projectId, repository]);

  const createEnvironment = async () => {
    const environment = newGithubEnvironment.trim();
    if (!targetValid || !/^[A-Za-z0-9._-]+$/.test(environment)) return;
    setCreatingEnvironment(true);
    try {
      const result = await api.createGitHubEnvironment(projectId, repository.trim(), environment);
      setGithubEnvironments(result.environments);
      setGithubEnvironment(environment);
      setNewGithubEnvironment("");
      setGithubTargetError(null);
      onNotice(t("push.environmentCreated", { name: environment }));
    } catch (error) {
      setGithubTargetError(localizeError(error, locale, "push.environmentCreateError"));
    } finally {
      setCreatingEnvironment(false);
    }
  };

  return {
    repository,
    setRepository,
    githubEnvironment,
    setGithubEnvironment,
    githubRepositories,
    githubEnvironments,
    newGithubEnvironment,
    setNewGithubEnvironment,
    loadingRepositories,
    loadingEnvironments,
    creatingEnvironment,
    githubTargetError,
    detectingRepository,
    targetValid,
    createEnvironment,
  };
}
