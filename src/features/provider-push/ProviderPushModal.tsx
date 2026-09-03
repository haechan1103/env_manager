import { useEffect, useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type {
  DeploymentProviderId,
  DeploymentProviderStatus,
  ProviderCompareResult,
  ProviderPushReceipt,
  ProjectProjection,
} from "../../lib/types";
import { ProviderSelector } from "./ProviderSelector";
import { ProviderTargetForm } from "./ProviderTargetForm";
import { ProviderPushResults } from "./ProviderPushResults";
import {
  ProviderVariableSelection,
  type ProviderSelection,
} from "./ProviderVariableSelection";
import { useGitHubTarget } from "./targets/useGitHubTarget";
import { useCloudflareTarget } from "./targets/useCloudflareTarget";
import { useEasTarget } from "./targets/useEasTarget";
import { useAwsTarget } from "./targets/useAwsTarget";
import { useRuntimeTarget } from "./targets/useRuntimeTarget";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function ProviderPushModal({ projectId, projection, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [providers, setProviders] = useState<DeploymentProviderStatus[]>([]);
  const [loadingProviders, setLoadingProviders] = useState(true);
  const [provider, setProvider] = useState<DeploymentProviderId>("github-actions");
  const [file, setFile] = useState(projection.files[0]?.path ?? "");
  const [personalTarget, setPersonalTarget] = useState("");
  const [installingPack, setInstallingPack] = useState(false);
  const [removingPack, setRemovingPack] = useState(false);
  const [selection, setSelection] = useState<ProviderSelection>({});
  const [busy, setBusy] = useState(false);
  const [comparing, setComparing] = useState(false);
  const [comparison, setComparison] = useState<ProviderCompareResult | null>(null);
  const [receipts, setReceipts] = useState<ProviderPushReceipt[]>([]);
  const [uiReady, setUiReady] = useState(false);
  const isAwsProvider = provider === "aws-secrets-manager" || provider === "aws-ssm-parameter-store";
  const isRemoteRuntime = provider === "remote-runtime";
  const githubAvailable = providers.find((item) => item.id === "github-actions")?.available ?? false;
  const github = useGitHubTarget({
    projectId,
    file,
    active: uiReady && provider === "github-actions",
    available: githubAvailable,
    onNotice,
  });
  const {
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
    targetValid: githubTargetValid,
    createEnvironment,
  } = github;
  const cloudflare = useCloudflareTarget({
    projectId,
    file,
    active: uiReady && provider === "cloudflare-workers",
    providers,
  });
  const {
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
  } = cloudflare;
  const eas = useEasTarget({
    projectId,
    file,
    active: uiReady && provider === "expo-eas",
  });
  const {
    easTarget,
    easProject,
    easEnvironments,
    setEasEnvironments,
    loadingEasTarget,
    easAccess,
    easAccessError,
  } = eas;
  const aws = useAwsTarget(uiReady && isAwsProvider);
  const {
    awsProfile,
    setAwsProfile,
    awsRegion,
    setAwsRegion,
    awsPathPrefix,
    setAwsPathPrefix,
    awsKmsKeyId,
    setAwsKmsKeyId,
    awsAccess,
    loadingAwsAccess,
    awsAccessError,
  } = aws;
  const runtimeTarget = useRuntimeTarget({
    projectId,
    file,
    setFile,
    active: uiReady && isRemoteRuntime,
    onError,
    onNotice,
  });
  const {
    runtimeTargets,
    runtimeTargetId,
    setRuntimeTargetId,
    editingRuntimeTarget,
    setEditingRuntimeTarget,
    runtimeDisplayName,
    setRuntimeDisplayName,
    runtimeRemoteId,
    setRuntimeRemoteId,
    runtimeDestination,
    setRuntimeDestination,
    runtimeRecipient,
    setRuntimeRecipient,
    savingRuntimeTarget,
    resetRuntimeTargetDraft,
    saveRuntimeTarget,
    removeRuntimeTarget,
  } = runtimeTarget;

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

  const isComparisonProvider = isAwsProvider || isRemoteRuntime;

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
    ? githubTargetValid
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
      <ProviderSelector
        providers={providers}
        selected={provider}
        loading={loadingProviders}
        runtimeTargets={runtimeTargets}
        installingPack={installingPack}
        onSelect={setProvider}
        onInstallPack={() => void installPack()}
      />

      <ProviderTargetForm
        provider={provider}
        isAwsProvider={isAwsProvider}
        isRemoteRuntime={isRemoteRuntime}
        source={{ projection, file, setFile }}
        github={{
          repository, setRepository, githubEnvironment, setGithubEnvironment,
          githubRepositories, githubEnvironments, newGithubEnvironment,
          setNewGithubEnvironment, loadingRepositories, loadingEnvironments,
          creatingEnvironment, detectingRepository, githubTargetError, targetValid,
          createEnvironment,
        }}
        cloudflare={{
          worker, setWorker, cloudflareEnvironment, setCloudflareEnvironment,
          cloudflareEnvironments, cloudflareConfigPath, loadingCloudflareTarget,
          cloudflareAccess, loadingCloudflareAccess, cloudflareAccessError,
        }}
        eas={{
          easTarget, easProject, easEnvironments, setEasEnvironments,
          loadingEasTarget, easAccess, easAccessError,
        }}
        aws={{
          awsProfile, setAwsProfile, awsRegion, setAwsRegion, awsPathPrefix,
          setAwsPathPrefix, awsKmsKeyId, setAwsKmsKeyId, awsAccess,
          loadingAwsAccess, awsAccessError,
        }}
        runtime={{
          runtimeTargets, runtimeTargetId, setRuntimeTargetId, editingRuntimeTarget,
          setEditingRuntimeTarget, runtimeDisplayName, setRuntimeDisplayName,
          runtimeRemoteId, setRuntimeRemoteId, runtimeDestination, setRuntimeDestination,
          runtimeRecipient, setRuntimeRecipient, savingRuntimeTarget,
          resetRuntimeTargetDraft, saveRuntimeTarget, removeRuntimeTarget,
          clearComparison: () => setComparison(null),
        }}
        personal={{
          currentProvider, personalTarget, setPersonalTarget, removingPack, busy, removePack,
        }}
      />

      <ProviderVariableSelection
        provider={provider}
        variables={variables}
        selection={selection}
        ready={uiReady}
        isAwsProvider={isAwsProvider}
        isRemoteRuntime={isRemoteRuntime}
        onChange={setSelection}
      />

      <ProviderPushResults
        provider={provider}
        selection={selection}
        isRemoteRuntime={isRemoteRuntime}
        latestReceipt={latestReceipt}
        comparison={comparison}
      />
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
