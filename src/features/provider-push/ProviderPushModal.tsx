import { useEffect, useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type {
  DeploymentProviderId,
  DeploymentProviderStatus,
  GitHubEntryKind,
  ProjectProjection,
} from "../../lib/types";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

type Selection = Record<string, { selected: boolean; kind: GitHubEntryKind }>;

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
  const [selection, setSelection] = useState<Selection>({});
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.listDeploymentProviders(projectId)
      .then(setProviders)
      .catch((error) => onError(localizeError(error, locale, "push.statusError")))
      .finally(() => setLoadingProviders(false));
  }, [locale, onError, projectId]);

  useEffect(() => setSelection({}), [file]);

  useEffect(() => {
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
  }, [file, projectId]);

  useEffect(() => {
    let active = true;
    setLoadingCloudflareTarget(true);
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
  }, [file, projectId]);

  const githubAvailable = providers.find((item) => item.id === "github-actions")?.available ?? false;

  useEffect(() => {
    if (!githubAvailable) return;
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
  }, [githubAvailable, locale, projectId]);

  useEffect(() => {
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
  }, [locale, projectId, repository]);

  const currentFile = projection.files.find((item) => item.path === file);
  const variables = useMemo(
    () => currentFile?.groups.flatMap((group) => group.variables) ?? [],
    [currentFile],
  );
  const selected = variables
    .filter((variable) => selection[variable.key]?.selected)
    .map((variable) => ({ key: variable.key, kind: selection[variable.key]?.kind ?? "secret" }));
  const available = providers.find((item) => item.id === provider)?.available ?? false;
  const targetValid = provider === "github-actions"
    ? /^[A-Za-z0-9._-]+\/[A-Za-z0-9._-]+$/.test(repository)
    : /^[A-Za-z0-9._-]+$/.test(worker);
  const githubEnvironmentReady = provider !== "github-actions" || githubEnvironment !== "__new__";
  const valid = available && selected.length > 0 && targetValid && githubEnvironmentReady;

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
      });
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

  return (
    <Modal title={t("push.title")} description={t("push.description")} onClose={onClose}>
      <div className="push-provider-options">
        {(["github-actions", "cloudflare-workers"] as DeploymentProviderId[]).map((id) => {
          const status = providers.find((item) => item.id === id);
          return (
            <button
              key={id}
              className={provider === id ? "push-provider selected" : "push-provider"}
              onClick={() => setProvider(id)}
            >
              <span className="push-provider-mark">{id === "github-actions" ? "GH" : "CF"}</span>
              <span>
                <strong>{id === "github-actions" ? "GitHub Actions" : "Cloudflare Workers"}</strong>
                <small className={loadingProviders ? "provider-status-loading" : undefined}>
                  {loadingProviders && <span className="spinner" />}
                  {loadingProviders ? t("push.checkingCli") : status?.available ? t("push.cliReady") : t("push.cliMissing")}
                </small>
              </span>
            </button>
          );
        })}
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
        ) : (
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
          </>
        )}
      </div>

      <section className="push-variable-section">
        <header>
          <div><strong>{t("push.selectVariables")}</strong><small>{t("push.valuesHidden")}</small></div>
          <button className="text-button" onClick={() => {
            const allSelected = variables.filter((item) => item.valueState === "present").every((item) => selection[item.key]?.selected);
            setSelection(Object.fromEntries(variables.map((item) => [item.key, {
              selected: item.valueState === "present" && !allSelected,
              kind: selection[item.key]?.kind ?? "secret",
            }])));
          }}>{t("push.selectAll")}</button>
        </header>
        <div className="push-variable-list">
          {variables.map((variable) => (
            <label className={variable.valueState === "empty" ? "push-variable disabled" : "push-variable"} key={variable.key}>
              <input
                type="checkbox"
                disabled={variable.valueState === "empty"}
                checked={selection[variable.key]?.selected ?? false}
                onChange={(event) => setSelection((current) => ({
                  ...current,
                  [variable.key]: { selected: event.target.checked, kind: current[variable.key]?.kind ?? "secret" },
                }))}
              />
              <span className="push-variable-name"><code>{variable.key}</code><small>{variable.valueState === "empty" ? t("push.empty") : t("push.valuePresent")}</small></span>
              {provider === "github-actions" ? (
                <select
                  aria-label={t("push.kindFor", { key: variable.key })}
                  disabled={!selection[variable.key]?.selected}
                  value={selection[variable.key]?.kind ?? "secret"}
                  onChange={(event) => setSelection((current) => ({
                    ...current,
                    [variable.key]: { selected: true, kind: event.target.value as GitHubEntryKind },
                  }))}
                >
                  <option value="secret">Secret</option>
                  <option value="variable">Variable</option>
                </select>
              ) : <span className="secret-only-badge">Secret</span>}
            </label>
          ))}
        </div>
      </section>

      <div className="provider-push-warning">
        <strong>{t("push.networkTitle")}</strong>
        <p>{t("push.networkBody")}</p>
        {provider === "github-actions" && selected.some((item) => item.kind === "variable") && (
          <p className="provider-variable-warning">{t("push.variableVisible")}</p>
        )}
      </div>
      <div className="modal-actions">
        <button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
        <button className="primary-button" disabled={!valid || busy} onClick={() => void submit()}>
          {busy ? t("push.pushing") : t("push.action", { count: selected.length })}
        </button>
      </div>
    </Modal>
  );
}
