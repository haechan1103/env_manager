import { useEffect, useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n, type TranslationKey } from "../../i18n";
import * as api from "../../lib/api";
import type {
  AwsAccessContext,
  CloudflareAccessContext,
  DeploymentProviderId,
  DeploymentProviderStatus,
  EasTargetContext,
  EasAccessContext,
  ProviderEntryKind,
  ProviderCompareResult,
  ProviderPushReceipt,
  ProjectProjection,
  RuntimeTarget,
} from "../../lib/types";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

type Selection = Record<string, { selected: boolean; kind: ProviderEntryKind }>;

export function ProviderPushModal({ projectId, projection, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [providers, setProviders] = useState<DeploymentProviderStatus[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [provider, setProvider] = useState<DeploymentProviderId>("github-actions");
  const [file, setFile] = useState(projection.files[0]?.path ?? "");
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
  const [worker, setWorker] = useState("");
  const [cloudflareEnvironment, setCloudflareEnvironment] = useState("");
  const [cloudflareEnvironments, setCloudflareEnvironments] = useState<string[]>([]);
  const [cloudflareConfigPath, setCloudflareConfigPath] = useState<string | null>(null);
  const [loadingCloudflareTarget, setLoadingCloudflareTarget] = useState(false);
  const [cloudflareAccess, setCloudflareAccess] = useState<CloudflareAccessContext | null>(null);
  const [loadingCloudflareAccess, setLoadingCloudflareAccess] = useState(false);
  const [cloudflareAccessError, setCloudflareAccessError] = useState(false);
  const [easTarget, setEasTarget] = useState<EasTargetContext | null>(null);
  const [easProject, setEasProject] = useState("");
  const [easEnvironments, setEasEnvironments] = useState<string[]>([]);
  const [loadingEasTarget, setLoadingEasTarget] = useState(false);
  const [easAccess, setEasAccess] = useState<EasAccessContext | null>(null);
  const [easAccessError, setEasAccessError] = useState(false);
  const [personalTarget, setPersonalTarget] = useState("");
  const [installingPack, setInstallingPack] = useState(false);
  const [removingPack, setRemovingPack] = useState(false);
  const [awsProfile, setAwsProfile] = useState("");
  const [awsRegion, setAwsRegion] = useState("");
  const [awsPathPrefix, setAwsPathPrefix] = useState("");
  const [awsKmsKeyId, setAwsKmsKeyId] = useState("");
  const [awsAccess, setAwsAccess] = useState<AwsAccessContext | null>(null);
  const [loadingAwsAccess, setLoadingAwsAccess] = useState(false);
  const [awsAccessError, setAwsAccessError] = useState(false);
  const [selection, setSelection] = useState<Selection>({});
  const [busy, setBusy] = useState(false);
  const [comparing, setComparing] = useState(false);
  const [comparison, setComparison] = useState<ProviderCompareResult | null>(null);
  const [receipts, setReceipts] = useState<ProviderPushReceipt[]>([]);
  const [runtimeTargets, setRuntimeTargets] = useState<RuntimeTarget[]>([]);
  const [runtimeTargetId, setRuntimeTargetId] = useState("");
  const [editingRuntimeTarget, setEditingRuntimeTarget] = useState(false);
  const [runtimeDisplayName, setRuntimeDisplayName] = useState("");
  const [runtimeRemoteId, setRuntimeRemoteId] = useState("");
  const [runtimeDestination, setRuntimeDestination] = useState("");
  const [runtimeRecipient, setRuntimeRecipient] = useState("");
  const [savingRuntimeTarget, setSavingRuntimeTarget] = useState(false);
  const [uiReady, setUiReady] = useState(false);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => setUiReady(true));
    return () => window.cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    if (!uiReady) return;
    let active = true;
    void api.listDeploymentProviders(projectId)
      .then((result) => { if (active) setProviders(result); })
      .catch((error) => { if (active) onError(localizeError(error, locale, "push.statusError")); })
      .finally(() => { if (active) setLoadingProviders(false); });
    return () => { active = false; };
  }, [locale, onError, projectId, uiReady]);

  useEffect(() => {
    if (!uiReady) return;
    let active = true;
    void api.listProviderPushReceipts(projectId)
      .then((result) => { if (active) setReceipts(result); })
      .catch(() => undefined);
    return () => { active = false; };
  }, [projectId, uiReady]);

  useEffect(() => {
    setSelection({});
    setComparison(null);
  }, [file]);

  useEffect(() => setComparison(null), [provider, awsProfile, awsRegion, awsPathPrefix]);

  useEffect(() => {
    setSelection((current) => Object.fromEntries(Object.entries(current).map(([key, item]) => [key, {
      ...item,
      kind: provider === "expo-eas"
        ? (item.kind === "plaintext" ? "plaintext" : "sensitive")
        : (item.kind === "variable" && provider === "github-actions" ? "variable" : "secret"),
    }])));
  }, [provider]);

  useEffect(() => {
    if (!uiReady || provider !== "remote-runtime") return;
    let active = true;
    void api.listRuntimeTargets(projectId)
      .then((targets) => {
        if (!active) return;
        setRuntimeTargets(targets);
        const selected = targets.find((target) => target.id === runtimeTargetId) ?? targets[0];
        if (selected) {
          setRuntimeTargetId(selected.id);
          setFile(selected.sourceFile);
        }
      })
      .catch((error) => onError(localizeError(error, locale, "compare.error")));
    return () => { active = false; };
  }, [locale, onError, projectId, provider, uiReady]);

  useEffect(() => {
    if (!uiReady || provider !== "github-actions") return;
    let active = true;
    setDetectingRepository(true);
    void api.detectGitHubRepository(projectId, file)
      .then((result) => {
        if (!active || !result.repository) return;
        setRepository(result.repository ?? "");
      })
      .catch(() => undefined)
      .finally(() => {
        if (active) setDetectingRepository(false);
      });
    return () => { active = false; };
  }, [file, projectId, provider, uiReady]);

  useEffect(() => {
    if (!uiReady || provider !== "expo-eas") return;
    let active = true;
    setLoadingEasTarget(true);
    setEasTarget(null);
    setEasProject("");
    setEasEnvironments([]);
    setEasAccess(null);
    setEasAccessError(false);
    void api.detectEasTarget(projectId, file)
      .then(async (target) => {
        if (!active) return;
        setEasTarget(target);
        const detectedProject = target.project ?? target.projectId ?? "";
        setEasProject(detectedProject);
        setEasEnvironments(target.environments);
        const access = await api.inspectEasAccess(projectId, file, detectedProject || null);
        if (active) setEasAccess(access);
      })
      .catch(() => {
        if (!active) return;
        setEasAccess(null);
        setEasAccessError(true);
      })
      .finally(() => { if (active) setLoadingEasTarget(false); });
    return () => { active = false; };
  }, [file, projectId, provider, uiReady]);

  useEffect(() => {
    if (!uiReady || provider !== "cloudflare-workers") return;
    let active = true;
    setLoadingCloudflareTarget(true);
    setCloudflareAccess(null);
    void api.detectCloudflareTarget(projectId, file)
      .then((result) => {
        if (!active) return;
        setWorker(result.worker ?? "");
        setCloudflareEnvironments(result.environments);
        setCloudflareEnvironment("");
        setCloudflareConfigPath(result.configPath);
      })
      .catch(() => {
        if (!active) return;
        setCloudflareEnvironments([]);
        setCloudflareConfigPath(null);
      })
      .finally(() => {
        if (active) setLoadingCloudflareTarget(false);
      });
    return () => { active = false; };
  }, [file, projectId, provider, uiReady]);

  useEffect(() => {
    const cloudflareAvailable = providers.find((item) => item.id === "cloudflare-workers")?.available ?? false;
    const normalizedWorker = worker.trim();
    const normalizedEnvironment = cloudflareEnvironment.trim();
    if (
      !uiReady
      || provider !== "cloudflare-workers"
      || !cloudflareAvailable
      || !/^[A-Za-z0-9._-]+$/.test(normalizedWorker)
      || loadingCloudflareTarget
    ) {
      setCloudflareAccess(null);
      setLoadingCloudflareAccess(false);
      setCloudflareAccessError(false);
      return;
    }
    let active = true;
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
          if (active) setCloudflareAccess(result);
        })
        .catch(() => {
          if (!active) return;
          setCloudflareAccess(null);
          setCloudflareAccessError(true);
        })
        .finally(() => {
          if (active) setLoadingCloudflareAccess(false);
        });
    }, 250);
    return () => {
      active = false;
      window.clearTimeout(timeout);
    };
  }, [cloudflareEnvironment, file, loadingCloudflareTarget, projectId, provider, providers, uiReady, worker]);

  const isAwsProvider = provider === "aws-secrets-manager" || provider === "aws-ssm-parameter-store";
  const isRemoteRuntime = provider === "remote-runtime";
  const isComparisonProvider = isAwsProvider || isRemoteRuntime;

  useEffect(() => {
    if (!uiReady || !isAwsProvider) {
      setAwsAccess(null);
      setLoadingAwsAccess(false);
      setAwsAccessError(false);
      return;
    }
    let active = true;
    setLoadingAwsAccess(true);
    setAwsAccessError(false);
    const timeout = window.setTimeout(() => {
      void api.inspectAwsAccess(awsProfile.trim() || null, awsRegion.trim() || null)
        .then((result) => {
          if (!active) return;
          setAwsAccess(result);
        })
        .catch(() => {
          if (!active) return;
          setAwsAccess(null);
          setAwsAccessError(true);
        })
        .finally(() => {
          if (active) setLoadingAwsAccess(false);
        });
    }, 350);
    return () => {
      active = false;
      window.clearTimeout(timeout);
    };
  }, [awsProfile, awsRegion, isAwsProvider, uiReady]);

  const githubAvailable = providers.find((item) => item.id === "github-actions")?.available ?? false;

  useEffect(() => {
    if (!uiReady || provider !== "github-actions" || !githubAvailable) return;
    let active = true;
    setLoadingRepositories(true);
    void api.listGitHubRepositories(projectId)
      .then((result) => {
        if (!active) return;
        setGithubRepositories(result.repositories);
        setGithubTargetError(null);
      })
      .catch((error) => {
        if (!active) return;
        setGithubTargetError(localizeError(error, locale, "push.targetsError"));
      })
      .finally(() => {
        if (active) setLoadingRepositories(false);
      });
    return () => { active = false; };
  }, [githubAvailable, locale, projectId, provider, uiReady]);

  useEffect(() => {
    if (!uiReady || provider !== "github-actions") return;
    const normalizedRepository = repository.trim();
    if (!/^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(normalizedRepository)) {
      setGithubEnvironments([]);
      setGithubEnvironment("");
      setLoadingEnvironments(false);
      return;
    }
    let active = true;
    setLoadingEnvironments(true);
    const timeout = window.setTimeout(() => {
      void api.listGitHubEnvironments(projectId, normalizedRepository)
        .then((result) => {
          if (!active || result.repository !== normalizedRepository) return;
          setGithubEnvironments(result.environments);
          setGithubEnvironment((current) => current === "__new__" || result.environments.includes(current) ? current : "");
          setGithubTargetError(null);
        })
        .catch((error) => {
          if (!active) return;
          setGithubEnvironments([]);
          setGithubTargetError(localizeError(error, locale, "push.targetsError"));
        })
        .finally(() => {
          if (active) setLoadingEnvironments(false);
        });
    }, 250);
    return () => {
      active = false;
      window.clearTimeout(timeout);
    };
  }, [locale, projectId, provider, repository, uiReady]);

  const currentFile = projection.files.find((item) => item.path === file);
  const variables = useMemo(
    () => uiReady ? currentFile?.groups.flatMap((group) => group.variables) ?? [] : [],
    [currentFile, uiReady],
  );
  const selected = variables
    .filter((variable) => selection[variable.key]?.selected)
    .map((variable) => ({ key: variable.key, kind: selection[variable.key]?.kind ?? (provider === "expo-eas" ? "sensitive" : "secret") }));
  const available = isRemoteRuntime
    ? runtimeTargets.length > 0
    : providers.find((item) => item.id === provider)?.available ?? false;
  const currentProvider = providers.find((item) => item.id === provider);
  const latestReceipt = receipts.find((item) => item.provider === provider && item.sourceFile === file) ?? null;
  const targetValid = provider === "github-actions"
    ? /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(repository)
    : provider === "cloudflare-workers"
      ? /^[A-Za-z0-9._-]+$/.test(worker)
      : provider === "expo-eas"
        ? easProject.trim().length > 0 && easEnvironments.length > 0 && easTarget !== null
      : isAwsProvider
        ? awsPathPrefix.length <= 400
        : isRemoteRuntime
          ? runtimeTargets.some((target) => target.id === runtimeTargetId && target.sourceFile === file)
        : currentProvider?.targetLabel
        ? personalTarget.trim().length > 0 && personalTarget.trim().length <= 128 && !personalTarget.trim().startsWith("-")
        : true;
  const githubEnvironmentReady = provider !== "github-actions" || githubEnvironment !== "__new__";
  const cloudflareReady = provider !== "cloudflare-workers" || (
    cloudflareAccess?.authState === "authenticated"
    && cloudflareAccess.accountState !== "mismatch"
    && cloudflareAccess.targetState === "accessible"
  );
  const awsReady = !isAwsProvider || (awsAccess !== null && !loadingAwsAccess);
  const easReady = provider !== "expo-eas" || (!loadingEasTarget && easTarget !== null && easAccess !== null);
  const valid = available && selected.length > 0 && targetValid && githubEnvironmentReady && cloudflareReady && awsReady && easReady;

  const resetRuntimeTargetDraft = () => {
    setRuntimeDisplayName("");
    setRuntimeRemoteId("");
    setRuntimeDestination("");
    setRuntimeRecipient("");
  };

  const saveRuntimeTarget = async () => {
    const remoteId = runtimeRemoteId.trim();
    const id = remoteId;
    if (!id || !runtimeDisplayName.trim() || !runtimeDestination.trim() || !runtimeRecipient.trim() || !file) return;
    setSavingRuntimeTarget(true);
    try {
      const targets = await api.saveRuntimeTarget(projectId, {
        id,
        displayName: runtimeDisplayName.trim(),
        sourceFile: file,
        remoteTargetId: remoteId,
        recipient: runtimeRecipient.trim(),
        transport: { type: "ssh", destination: runtimeDestination.trim() },
      });
      setRuntimeTargets(targets);
      setRuntimeTargetId(id);
      setEditingRuntimeTarget(false);
      resetRuntimeTargetDraft();
      onNotice(t("runtimeTarget.saved"));
    } catch (error) {
      onError(localizeError(error, locale, "runtimeTarget.saveError"));
    } finally {
      setSavingRuntimeTarget(false);
    }
  };

  const removeRuntimeTarget = async () => {
    if (!runtimeTargetId) return;
    setSavingRuntimeTarget(true);
    try {
      const targets = await api.removeRuntimeTarget(projectId, runtimeTargetId);
      setRuntimeTargets(targets);
      const next = targets[0];
      setRuntimeTargetId(next?.id ?? "");
      if (next) setFile(next.sourceFile);
      onNotice(t("runtimeTarget.removed"));
    } catch (error) {
      onError(localizeError(error, locale, "runtimeTarget.removeError"));
    } finally {
      setSavingRuntimeTarget(false);
    }
  };

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

  const installPack = async () => {
    setInstallingPack(true);
    try {
      const installed = await api.chooseAndInstallPersonalProviderPack(t("push.installPackTitle"));
      if (!installed) return;
      const next = await api.listDeploymentProviders(projectId);
      setProviders(next);
      setProvider(installed.id);
      setPersonalTarget("");
      onNotice(t("push.packInstalled", { name: installed.displayName, version: installed.version }));
    } catch (error) {
      onError(localizeError(error, locale, "push.packInstallError"));
    } finally {
      setInstallingPack(false);
    }
  };

  const removePack = async () => {
    if (currentProvider?.source !== "personal") return;
    setRemovingPack(true);
    try {
      await api.removePersonalProviderPack(currentProvider.id);
      const next = await api.listDeploymentProviders(projectId);
      setProviders(next);
      setProvider(next.find((item) => item.id === "github-actions")?.id ?? next[0]?.id ?? "github-actions");
      setPersonalTarget("");
      onNotice(t("push.packRemoved", { name: currentProvider.name }));
    } catch (error) {
      onError(localizeError(error, locale, "push.packRemoveError"));
    } finally {
      setRemovingPack(false);
    }
  };

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const result = await api.pushToProvider(projectId, {
        provider,
        file,
        selections: selected,
        repository: provider === "github-actions" ? repository.trim() : null,
        githubEnvironment: provider === "github-actions" && githubEnvironment !== "__new__" ? githubEnvironment.trim() || null : null,
        worker: provider === "cloudflare-workers" ? worker.trim() : null,
        cloudflareEnvironment: provider === "cloudflare-workers" ? cloudflareEnvironment.trim() || null : null,
        easProject: provider === "expo-eas" ? easProject.trim() || null : null,
        easEnvironments: provider === "expo-eas" ? easEnvironments : [],
        personalTarget: currentProvider?.source === "personal" ? personalTarget.trim() || null : null,
        awsProfile: isAwsProvider ? awsProfile.trim() || null : null,
        awsRegion: isAwsProvider ? awsRegion.trim() || null : null,
        awsPathPrefix: isAwsProvider ? awsPathPrefix.trim() || null : null,
        awsKmsKeyId: isAwsProvider ? awsKmsKeyId.trim() || null : null,
      });
      setReceipts(await api.listProviderPushReceipts(projectId));
      if (result.failedKeys.length > 0) {
        onError(t("push.partial", {
          pushed: result.pushedCount,
          failed: result.failedKeys.join(", "),
        }));
      } else {
        onNotice(t("push.done", { count: result.pushedCount }));
        onClose();
      }
    } catch (error) {
      onError(localizeError(error, locale, "push.error"));
    } finally {
      setBusy(false);
    }
  };

  const compare = async () => {
    if (!isComparisonProvider || !valid) return;
    setComparing(true);
    setComparison(null);
    try {
      const result = await api.compareProviderValues(projectId, {
        provider,
        file,
        keys: selected.map((item) => item.key),
        awsProfile: awsProfile.trim() || null,
        awsRegion: awsRegion.trim() || null,
        awsPathPrefix: awsPathPrefix.trim() || null,
        runtimeTargetId: isRemoteRuntime ? runtimeTargetId : null,
      });
      setComparison(result);
    } catch (error) {
      onError(localizeError(error, locale, "compare.error"));
    } finally {
      setComparing(false);
    }
  };

  return (
    <Modal title={t("push.title")} description={t("push.description")} onClose={onClose}>
      <div className="push-provider-options">
        {(providers.length > 0 ? providers : [
          { id: "github-actions", name: "GitHub Actions" },
          { id: "cloudflare-workers", name: "Cloudflare Workers" },
        ]).map((item) => {
          const id = item.id as DeploymentProviderId;
          const status = providers.find((candidate) => candidate.id === id);
          const mark = id === "github-actions" ? "GH"
              : id === "cloudflare-workers" ? "CF"
              : id === "expo-eas" ? "EA"
              : id === "aws-secrets-manager" ? "AS"
                : id === "aws-ssm-parameter-store" ? "SS"
                  : id === "remote-runtime" ? "RT" : "P";
          return (
            <button
              key={id}
              className={provider === id ? "push-provider selected" : "push-provider"}
              onClick={() => setProvider(id)}
            >
              <span className="push-provider-mark">{mark}</span>
              <span>
                <strong>{item.name}</strong>
                <small className={loadingProviders ? "provider-status-loading" : undefined}>
                  {loadingProviders && <span className="spinner" />}
                  {loadingProviders ? t("push.checkingCli") : id === "remote-runtime"
                    ? (runtimeTargets.length > 0 ? t("runtimeTarget.ready") : t("runtimeTarget.missing"))
                    : status?.available ? (
                    <><span>{t("push.cliReady")}</span>{status.adapter && ` · v${status.adapter.cliVersion}`}</>
                  ) : t("push.cliMissing")}
                </small>
                {status?.source === "personal" && <small>{t("push.personalPack", { version: status.version ?? "" })}</small>}
              </span>
            </button>
          );
        })}
        <button className="push-provider add-pack" disabled={installingPack} onClick={() => void installPack()}>
          <span className="push-provider-mark">+</span>
          <span><strong>{t("push.addPack")}</strong><small>{t("push.addPackBody")}</small></span>
        </button>
      </div>

      <div className="modal-form provider-target-form">
        <label>
          <span>{t("push.sourceFile")}</span>
          <select value={file} onChange={(event) => {
            setRepository("");
            setFile(event.target.value);
          }}>
            {projection.files.map((item) => <option key={item.path} value={item.path}>{item.displayName}</option>)}
          </select>
        </label>
        {provider === "github-actions" ? (
          <>
            <label>
              <span>{t("push.repository")}</span>
              <input
                aria-label={t("push.repository")}
                value={repository}
                list="github-repository-options"
                placeholder="owner/repository"
                autoComplete="off"
                onChange={(event) => setRepository(event.target.value)}
              />
              <datalist id="github-repository-options">
                {githubRepositories.map((name) => <option key={name} value={name} />)}
              </datalist>
              <small className="field-help">
                {!loadingRepositories && t("push.repositoryHelp", { count: githubRepositories.length })}
              </small>
              {(detectingRepository || loadingRepositories) && (
                <span className="target-loading" role="status">
                  <span className="spinner" />
                  {detectingRepository ? t("push.detectingRepository") : t("push.loadingRepositories")}
                </span>
              )}
            </label>
            <label>
              <span>{t("push.githubEnvironment")} <em>{t("common.optional")}</em></span>
              <select
                aria-label={t("push.githubEnvironment")}
                value={githubEnvironment}
                disabled={!targetValid || loadingEnvironments}
                onChange={(event) => setGithubEnvironment(event.target.value)}
              >
                <option value="">{loadingEnvironments ? t("push.loadingEnvironments") : t("push.repositoryScope")}</option>
                {githubEnvironments.map((name) => <option key={name} value={name}>{name}</option>)}
                <option value="__new__">{t("push.createEnvironment")}</option>
              </select>
              {githubEnvironment === "__new__" && (
                <span className="github-environment-create">
                  <input
                    aria-label={t("push.newEnvironmentName")}
                    value={newGithubEnvironment}
                    placeholder="production"
                    onChange={(event) => setNewGithubEnvironment(event.target.value)}
                  />
                  <button
                    className="quiet-button"
                    disabled={creatingEnvironment || !/^[A-Za-z0-9._-]+$/.test(newGithubEnvironment.trim())}
                    onClick={() => void createEnvironment()}
                  >
                    {creatingEnvironment ? t("push.creatingEnvironment") : t("push.addEnvironment")}
                  </button>
                </span>
              )}
              {loadingEnvironments && (
                <span className="target-loading" role="status">
                  <span className="spinner" />
                  {t("push.loadingEnvironments")}
                </span>
              )}
            </label>
            {githubTargetError && <p className="field-error github-target-error">{githubTargetError}</p>}
          </>
        ) : provider === "cloudflare-workers" ? (
          <>
            <label>
              <span>{t("push.worker")}</span>
              <input value={worker} placeholder="my-worker" onChange={(event) => setWorker(event.target.value)} />
              {loadingCloudflareTarget && (
                <span className="target-loading" role="status"><span className="spinner" />{t("push.detectingCloudflare")}</span>
              )}
              {!loadingCloudflareTarget && cloudflareConfigPath && (
                <small className="field-help">{t("push.cloudflareDetected", { path: cloudflareConfigPath })}</small>
              )}
            </label>
            <label>
              <span>{t("push.cloudflareEnvironment")} <em>{t("common.optional")}</em></span>
              <input
                value={cloudflareEnvironment}
                list="cloudflare-environment-options"
                placeholder={t("push.cloudflareDefaultEnvironment")}
                onChange={(event) => setCloudflareEnvironment(event.target.value)}
              />
              <datalist id="cloudflare-environment-options">
                {cloudflareEnvironments.map((name) => <option key={name} value={name} />)}
              </datalist>
              {!loadingCloudflareTarget && cloudflareConfigPath && (
                <small className="field-help">{t("push.cloudflareEnvironmentHelp", { count: cloudflareEnvironments.length })}</small>
              )}
            </label>
            <CloudflareAccessStatus
              access={cloudflareAccess}
              loading={loadingCloudflareAccess}
              failed={cloudflareAccessError}
              t={t}
            />
          </>
        ) : provider === "expo-eas" ? (
          <>
            <label>
              <span>{t("push.easProject")}</span>
              <input value={easProject} readOnly placeholder="travel-pieces" />
              {loadingEasTarget ? (
                <span className="target-loading" role="status"><span className="spinner" />{t("push.detectingEas")}</span>
              ) : easTarget?.configPath ? (
                <small className="field-help">{t("push.easDetected", { path: easTarget.configPath })}</small>
              ) : (
                <small className="field-error">{t("push.easTargetMissing")}</small>
              )}
              {!loadingEasTarget && easAccess && (
                <small className="field-help">{t("push.easAccessReady", { project: easAccess.project })}</small>
              )}
              {!loadingEasTarget && easAccessError && easTarget && (
                <small className="field-error">{t("push.easAccessFailed")}</small>
              )}
            </label>
            <fieldset className="eas-environment-options">
              <legend>{t("push.easEnvironments")}</legend>
              {(easTarget?.environments ?? []).map((environment) => (
                <label key={environment}>
                  <input
                    type="checkbox"
                    checked={easEnvironments.includes(environment)}
                    onChange={(event) => setEasEnvironments((current) => event.target.checked
                      ? [...new Set([...current, environment])]
                      : current.filter((item) => item !== environment))}
                  />
                  <span>{environment}</span>
                </label>
              ))}
              <small>{t("push.easEnvironmentHelp")}</small>
            </fieldset>
          </>
        ) : isAwsProvider ? (
          <>
            <label>
              <span>{t("push.awsProfile")} <em>{t("common.optional")}</em></span>
              <input
                value={awsProfile}
                placeholder="default"
                autoComplete="off"
                onChange={(event) => setAwsProfile(event.target.value)}
              />
            </label>
            <label>
              <span>{t("push.awsRegion")} <em>{t("common.optional")}</em></span>
              <input
                value={awsRegion}
                placeholder="ap-northeast-2"
                autoComplete="off"
                onChange={(event) => setAwsRegion(event.target.value)}
              />
            </label>
            <label>
              <span>{provider === "aws-secrets-manager" ? t("push.awsSecretPrefix") : t("push.awsParameterPrefix")} <em>{t("common.optional")}</em></span>
              <input
                value={awsPathPrefix}
                placeholder="my-service/staging"
                autoComplete="off"
                onChange={(event) => setAwsPathPrefix(event.target.value)}
              />
              <small className="field-help">{t("push.awsPrefixHelp")}</small>
            </label>
            <label>
              <span>{t("push.awsKmsKey")} <em>{t("common.optional")}</em></span>
              <input
                value={awsKmsKeyId}
                list="aws-kms-alias-options"
                placeholder={provider === "aws-secrets-manager" ? "aws/secretsmanager" : "alias/aws/ssm"}
                autoComplete="off"
                onChange={(event) => setAwsKmsKeyId(event.target.value)}
              />
              <datalist id="aws-kms-alias-options">
                {awsAccess?.kmsAliases.map((alias) => <option key={alias} value={alias} />)}
              </datalist>
              <small className="field-help">{t("push.awsKmsHelp")}</small>
            </label>
            <AwsAccessStatus access={awsAccess} loading={loadingAwsAccess} failed={awsAccessError} t={t} />
          </>
        ) : isRemoteRuntime ? (
          <>
            <label>
              <span>{t("runtimeTarget.target")}</span>
              <select
                value={runtimeTargetId}
                onChange={(event) => {
                  const nextId = event.target.value;
                  setRuntimeTargetId(nextId);
                  const next = runtimeTargets.find((target) => target.id === nextId);
                  if (next) setFile(next.sourceFile);
                  setComparison(null);
                }}
              >
                <option value="">{t("runtimeTarget.select")}</option>
                {runtimeTargets.map((target) => (
                  <option key={target.id} value={target.id}>
                    {target.displayName} · {target.transport.type.toUpperCase()}
                  </option>
                ))}
              </select>
            </label>
            <div className="runtime-target-actions">
              <button
                className="quiet-button compact"
                onClick={() => {
                  resetRuntimeTargetDraft();
                  setEditingRuntimeTarget((current) => !current);
                }}
              >
                {editingRuntimeTarget ? t("common.cancel") : t("runtimeTarget.add")}
              </button>
              {runtimeTargetId && (
                <button
                  className="danger-quiet-button compact"
                  disabled={savingRuntimeTarget}
                  onClick={() => void removeRuntimeTarget()}
                >
                  {t("common.remove")}
                </button>
              )}
            </div>
            {editingRuntimeTarget && (
              <div className="runtime-target-editor">
                <label>
                  <span>{t("runtimeTarget.displayName")}</span>
                  <input value={runtimeDisplayName} placeholder="mobile-ok · dev" onChange={(event) => setRuntimeDisplayName(event.target.value)} />
                </label>
                <label>
                  <span>{t("runtimeTarget.remoteId")}</span>
                  <input value={runtimeRemoteId} placeholder="mobile-ok-dev" onChange={(event) => setRuntimeRemoteId(event.target.value)} />
                </label>
                <label>
                  <span>{t("runtimeTarget.sshDestination")}</span>
                  <input value={runtimeDestination} placeholder="deploy@example.com" onChange={(event) => setRuntimeDestination(event.target.value)} />
                </label>
                <label>
                  <span>{t("runtimeTarget.recipient")}</span>
                  <input value={runtimeRecipient} placeholder="age1…" onChange={(event) => setRuntimeRecipient(event.target.value)} />
                  <small className="field-help">{t("runtimeTarget.recipientHelp")}</small>
                </label>
                <button
                  className="primary-button"
                  disabled={savingRuntimeTarget || !runtimeDisplayName.trim() || !runtimeRemoteId.trim() || !runtimeDestination.trim() || !runtimeRecipient.trim()}
                  onClick={() => void saveRuntimeTarget()}
                >
                  {savingRuntimeTarget ? t("common.saving") : t("common.save")}
                </button>
              </div>
            )}
            <p className="field-help runtime-target-help">{t("runtimeTarget.help")}</p>
          </>
        ) : currentProvider?.source === "personal" ? (
          <>
            {currentProvider.targetLabel && (
              <label>
                <span>{currentProvider.targetLabel}</span>
                <input
                  value={personalTarget}
                  maxLength={128}
                  autoComplete="off"
                  onChange={(event) => setPersonalTarget(event.target.value)}
                />
              </label>
            )}
            <div className="personal-provider-trust">
              <div>
                <strong>{t("push.personalTrustTitle")}</strong>
                <p>{t("push.personalTrustBody")}</p>
              </div>
              <button
                className="danger-quiet-button compact"
                disabled={removingPack || busy}
                onClick={() => void removePack()}
              >
                {removingPack ? t("push.removingPack") : t("push.removePack")}
              </button>
            </div>
          </>
        ) : null}
      </div>

      <section className="push-variable-section">
        <header>
          <div><strong>{t("push.selectVariables")}</strong><small>{t("push.valuesHidden")}</small></div>
          <button className="text-button" onClick={() => {
            const allSelected = variables.filter((item) => item.valueState === "present").every((item) => selection[item.key]?.selected);
            setSelection(Object.fromEntries(variables.map((item) => [item.key, {
              selected: item.valueState === "present" && !allSelected,
              kind: selection[item.key]?.kind ?? (provider === "expo-eas" ? "sensitive" : "secret"),
            }])));
          }}>{t("push.selectAll")}</button>
        </header>
        <div className="push-variable-list">
          {!uiReady ? (
            <div className="push-variable-loading" role="status"><span className="spinner" />{t("push.preparing")}</div>
          ) : variables.map((variable) => (
            <label className={variable.valueState === "empty" ? "push-variable disabled" : "push-variable"} key={variable.key}>
              <input
                type="checkbox"
                disabled={variable.valueState === "empty"}
                checked={selection[variable.key]?.selected ?? false}
                onChange={(event) => setSelection((current) => ({
                  ...current,
                  [variable.key]: { selected: event.target.checked, kind: current[variable.key]?.kind ?? (provider === "expo-eas" ? "sensitive" : "secret") },
                }))}
              />
              <span className="push-variable-name"><code>{variable.key}</code><small>{variable.valueState === "empty" ? t("push.empty") : t("push.valuePresent")}</small></span>
              {provider === "github-actions" ? (
                <select
                  aria-label={t("push.kindFor", { key: variable.key })}
                  value={selection[variable.key]?.kind ?? "secret"}
                  onChange={(event) => setSelection((current) => ({
                    ...current,
                    [variable.key]: {
                      selected: current[variable.key]?.selected ?? false,
                      kind: event.target.value as ProviderEntryKind,
                    },
                  }))}
                >
                  <option value="secret">Secret</option>
                  <option value="variable">Variable</option>
                </select>
              ) : provider === "expo-eas" ? (
                <select
                  aria-label={t("push.kindFor", { key: variable.key })}
                  value={selection[variable.key]?.kind ?? "sensitive"}
                  onChange={(event) => setSelection((current) => ({
                    ...current,
                    [variable.key]: {
                      selected: current[variable.key]?.selected ?? false,
                      kind: event.target.value as ProviderEntryKind,
                    },
                  }))}
                >
                  <option value="sensitive">Sensitive</option>
                  <option value="plaintext">Plain text</option>
                  {!variable.key.startsWith("EXPO_PUBLIC_") && <option value="secret">Secret</option>}
                </select>
              ) : <span className="secret-only-badge">{provider === "cloudflare-workers" ? t("push.workerSecret") : isAwsProvider ? t("push.awsSecretType") : isRemoteRuntime ? t("runtimeTarget.encryptedCompare") : t("push.stdinSecret")}</span>}
            </label>
          ))}
        </div>
      </section>

      {latestReceipt && (
        <section className="provider-push-receipt">
          <div>
            <strong>{t("pushReceipt.title")}</strong>
            <span>{latestReceipt.destination}</span>
          </div>
          <div>
            <strong>{t("pushReceipt.succeeded", { count: latestReceipt.succeededKeys.length })}</strong>
            <span>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(latestReceipt.timestampMs)}</span>
          </div>
          <p>{t("pushReceipt.notEquality")}</p>
        </section>
      )}

      <div className="provider-push-warning">
        <strong>{t(isRemoteRuntime ? "runtimeTarget.networkTitle" : "push.networkTitle")}</strong>
        <p>{t(isRemoteRuntime ? "runtimeTarget.networkBody" : "push.networkBody")}</p>
        {provider === "github-actions" && Object.values(selection).some((item) => item.kind === "variable") && (
          <p className="provider-variable-warning">{t("push.variableVisible")}</p>
        )}
        {provider === "expo-eas" && (
          <p className="provider-variable-warning">{t("push.easVisibilityHelp")}</p>
        )}
      </div>
      {comparison && (
        <section className="provider-comparison" aria-live="polite">
          <header>
            <div>
              <strong>{t("compare.resultTitle")}</strong>
              <small>{t("compare.target", { target: comparison.target })}</small>
            </div>
            <span>{t("compare.checkedNow")}</span>
          </header>
          <div className="provider-comparison-list">
            {comparison.items.map((item) => (
              <div className={`provider-comparison-row ${item.state}`} key={item.key}>
                <span><code>{item.key}</code><small>{item.remoteName}</small></span>
                <strong>{t(comparisonStateKey(item.state))}</strong>
              </div>
            ))}
          </div>
          <p>{t("compare.redactedHelp")}</p>
        </section>
      )}
      <div className="modal-actions">
        <button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
        {isComparisonProvider && (
          <button className="quiet-button" disabled={!valid || busy || comparing} onClick={() => void compare()}>
            {comparing ? t("compare.checking") : t("compare.action", { count: selected.length })}
          </button>
        )}
        {!isRemoteRuntime && (
          <button className="primary-button" disabled={!valid || busy} onClick={() => void submit()}>
            {busy ? t("push.pushing") : t("push.action", { count: selected.length })}
          </button>
        )}
      </div>
    </Modal>
  );
}

function comparisonStateKey(state: ProviderCompareResult["items"][number]["state"]): TranslationKey {
  switch (state) {
    case "same": return "compare.state.same";
    case "different": return "compare.state.different";
    case "unset": return "compare.state.unset";
    case "unverifiable": return "compare.state.unverifiable";
    case "error": return "compare.state.error";
  }
}

function CloudflareAccessStatus({
  access,
  loading,
  failed,
  t,
}: {
  access: CloudflareAccessContext | null;
  loading: boolean;
  failed: boolean;
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (loading) {
    return (
      <div className="cloudflare-access-status checking" role="status">
        <span className="spinner" />{t("push.cloudflareCheckingAccess")}
      </div>
    );
  }
  if (failed || access?.authState === "unavailable") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareAccessCheckFailed")}</div>;
  }
  if (!access) return null;
  if (access.authState === "not-authenticated") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareNotAuthenticated")}</div>;
  }
  if (access.accountState === "mismatch") {
    return (
      <div className="cloudflare-access-status error">
        {t("push.cloudflareAccountMismatch", { account: access.accountId ?? "-" })}
      </div>
    );
  }
  if (access.targetState !== "accessible") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareTargetUnavailable")}</div>;
  }
  const account = access.accountName
    ? `${access.accountName}${access.accountId ? ` · ${access.accountId}` : ""}`
    : access.accountId ?? t("push.cloudflareWranglerSelectedAccount");
  return (
    <div className="cloudflare-access-status ready">
      <strong>{t("push.cloudflareAccessReady")}</strong>
      <span>{account}</span>
      {access.accountState === "ambiguous" && (
        <small>{t("push.cloudflareAccountAmbiguous", { count: access.accountCount })}</small>
      )}
      {access.adapter.adapterSource === "local-repair" && <small>{t("push.localRepairAdapter")}</small>}
    </div>
  );
}

function AwsAccessStatus({
  access,
  loading,
  failed,
  t,
}: {
  access: AwsAccessContext | null;
  loading: boolean;
  failed: boolean;
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (loading) {
    return (
      <div className="cloudflare-access-status checking" role="status">
        <span className="spinner" />{t("push.awsCheckingAccess")}
      </div>
    );
  }
  if (failed) {
    return <div className="cloudflare-access-status error">{t("push.awsAccessFailed")}</div>;
  }
  if (!access) return null;
  return (
    <div className="cloudflare-access-status ready">
      <strong>{t("push.awsAccessReady")}</strong>
      <span>{access.accountId} · {access.region}</span>
      {!access.kmsAliasesAvailable && <small>{t("push.awsKmsListUnavailable")}</small>}
    </div>
  );
}
