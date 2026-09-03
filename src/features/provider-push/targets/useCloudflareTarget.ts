import { useEffect, useState } from "react";

import * as api from "../../../lib/api";
import type { CloudflareAccessContext, DeploymentProviderStatus } from "../../../lib/types";

interface Options {
  projectId: string;
  file: string;
  active: boolean;
  providers: DeploymentProviderStatus[];
}

export function useCloudflareTarget({ projectId, file, active, providers }: Options) {
  const [worker, setWorker] = useState("");
  const [cloudflareEnvironment, setCloudflareEnvironment] = useState("");
  const [cloudflareEnvironments, setCloudflareEnvironments] = useState<string[]>([]);
  const [cloudflareConfigPath, setCloudflareConfigPath] = useState<string | null>(null);
  const [loadingCloudflareTarget, setLoadingCloudflareTarget] = useState(false);
  const [cloudflareAccess, setCloudflareAccess] = useState<CloudflareAccessContext | null>(null);
  const [loadingCloudflareAccess, setLoadingCloudflareAccess] = useState(false);
  const [cloudflareAccessError, setCloudflareAccessError] = useState(false);

  useEffect(() => {
    if (!active) return;
    let current = true;
    setLoadingCloudflareTarget(true);
    setCloudflareAccess(null);
    void api.detectCloudflareTarget(projectId, file)
      .then((result) => {
        if (!current) return;
        setWorker(result.worker ?? "");
        setCloudflareEnvironments(result.environments);
        setCloudflareEnvironment("");
        setCloudflareConfigPath(result.configPath);
      })
      .catch(() => {
        if (!current) return;
        setCloudflareEnvironments([]);
        setCloudflareConfigPath(null);
      })
      .finally(() => {
        if (current) setLoadingCloudflareTarget(false);
      });
    return () => { current = false; };
  }, [active, file, projectId]);

  useEffect(() => {
    const available = providers.find((item) => item.id === "cloudflare-workers")?.available ?? false;
    const normalizedWorker = worker.trim();
    const normalizedEnvironment = cloudflareEnvironment.trim();
    if (
      !active
      || !available
      || !/^[A-Za-z0-9._-]+$/.test(normalizedWorker)
      || loadingCloudflareTarget
    ) {
      setCloudflareAccess(null);
      setLoadingCloudflareAccess(false);
      setCloudflareAccessError(false);
      return;
    }
    let current = true;
    setLoadingCloudflareAccess(true);
    setCloudflareAccessError(false);
    const timeout = window.setTimeout(() => {
      void api.inspectCloudflareAccess(
        projectId,
        file,
        normalizedWorker,
        normalizedEnvironment || null,
      )
        .then((result) => {
          if (current) setCloudflareAccess(result);
        })
        .catch(() => {
          if (!current) return;
          setCloudflareAccess(null);
          setCloudflareAccessError(true);
        })
        .finally(() => {
          if (current) setLoadingCloudflareAccess(false);
        });
    }, 250);
    return () => {
      current = false;
      window.clearTimeout(timeout);
    };
  }, [active, cloudflareEnvironment, file, loadingCloudflareTarget, projectId, providers, worker]);

  return {
    worker,
    setWorker,
    cloudflareEnvironment,
    setCloudflareEnvironment,
    cloudflareEnvironments,
    cloudflareConfigPath,
    loadingCloudflareTarget,
    cloudflareAccess,
    loadingCloudflareAccess,
    cloudflareAccessError,
  };
}
