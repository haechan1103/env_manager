import type { Dispatch, SetStateAction } from "react";

import { useI18n } from "../../i18n";
import type {
  AwsAccessContext,
  CloudflareAccessContext,
  DeploymentProviderId,
  DeploymentProviderStatus,
  EasAccessContext,
  EasTargetContext,
  ProjectProjection,
  RuntimeTarget,
} from "../../lib/types";
import { AwsAccessStatus, CloudflareAccessStatus } from "./ProviderAccessStatus";

type Setter<T> = Dispatch<SetStateAction<T>>;

interface Props {
  provider: DeploymentProviderId;
  isAwsProvider: boolean;
  isRemoteRuntime: boolean;
  source: {
    projection: ProjectProjection;
    file: string;
    setFile: Setter<string>;
  };
  github: {
    repository: string;
    setRepository: Setter<string>;
    githubEnvironment: string;
    setGithubEnvironment: Setter<string>;
    githubRepositories: string[];
    githubEnvironments: string[];
    newGithubEnvironment: string;
    setNewGithubEnvironment: Setter<string>;
    loadingRepositories: boolean;
    loadingEnvironments: boolean;
    creatingEnvironment: boolean;
    detectingRepository: boolean;
    githubTargetError: string | null;
    targetValid: boolean;
    createEnvironment: () => Promise<void>;
  };
  cloudflare: {
    worker: string;
    setWorker: Setter<string>;
    cloudflareEnvironment: string;
    setCloudflareEnvironment: Setter<string>;
    cloudflareEnvironments: string[];
    cloudflareConfigPath: string | null;
    loadingCloudflareTarget: boolean;
    cloudflareAccess: CloudflareAccessContext | null;
    loadingCloudflareAccess: boolean;
    cloudflareAccessError: boolean;
  };
  eas: {
    easTarget: EasTargetContext | null;
    easProject: string;
    easEnvironments: string[];
    setEasEnvironments: Setter<string[]>;
    loadingEasTarget: boolean;
    easAccess: EasAccessContext | null;
    easAccessError: boolean;
  };
  aws: {
    awsProfile: string;
    setAwsProfile: Setter<string>;
    awsRegion: string;
    setAwsRegion: Setter<string>;
    awsPathPrefix: string;
    setAwsPathPrefix: Setter<string>;
    awsKmsKeyId: string;
    setAwsKmsKeyId: Setter<string>;
    awsAccess: AwsAccessContext | null;
    loadingAwsAccess: boolean;
    awsAccessError: boolean;
  };
  runtime: {
    runtimeTargets: RuntimeTarget[];
    runtimeTargetId: string;
    setRuntimeTargetId: Setter<string>;
    editingRuntimeTarget: boolean;
    setEditingRuntimeTarget: Setter<boolean>;
    runtimeDisplayName: string;
    setRuntimeDisplayName: Setter<string>;
    runtimeRemoteId: string;
    setRuntimeRemoteId: Setter<string>;
    runtimeDestination: string;
    setRuntimeDestination: Setter<string>;
    runtimeRecipient: string;
    setRuntimeRecipient: Setter<string>;
    savingRuntimeTarget: boolean;
    resetRuntimeTargetDraft: () => void;
    saveRuntimeTarget: () => Promise<void>;
    removeRuntimeTarget: () => Promise<void>;
    clearComparison: () => void;
  };
  personal: {
    currentProvider: DeploymentProviderStatus | undefined;
    personalTarget: string;
    setPersonalTarget: Setter<string>;
    removingPack: boolean;
    busy: boolean;
    removePack: () => Promise<void>;
  };
}

export function ProviderTargetForm({
  provider,
  isAwsProvider,
  isRemoteRuntime,
  source,
  github,
  cloudflare,
  eas,
  aws,
  runtime,
  personal,
}: Props) {
  const { t } = useI18n();
  const { projection, file, setFile } = source;
  const {
    repository, setRepository, githubEnvironment, setGithubEnvironment,
    githubRepositories, githubEnvironments, newGithubEnvironment,
    setNewGithubEnvironment, loadingRepositories, loadingEnvironments,
    creatingEnvironment, detectingRepository, githubTargetError, targetValid,
    createEnvironment,
  } = github;
  const {
    worker, setWorker, cloudflareEnvironment, setCloudflareEnvironment,
    cloudflareEnvironments, cloudflareConfigPath, loadingCloudflareTarget,
    cloudflareAccess, loadingCloudflareAccess, cloudflareAccessError,
  } = cloudflare;
  const {
    easTarget, easProject, easEnvironments, setEasEnvironments, loadingEasTarget,
    easAccess, easAccessError,
  } = eas;
  const {
    awsProfile, setAwsProfile, awsRegion, setAwsRegion, awsPathPrefix,
    setAwsPathPrefix, awsKmsKeyId, setAwsKmsKeyId, awsAccess,
    loadingAwsAccess, awsAccessError,
  } = aws;
  const {
    runtimeTargets, runtimeTargetId, setRuntimeTargetId, editingRuntimeTarget,
    setEditingRuntimeTarget, runtimeDisplayName, setRuntimeDisplayName,
    runtimeRemoteId, setRuntimeRemoteId, runtimeDestination, setRuntimeDestination,
    runtimeRecipient, setRuntimeRecipient, savingRuntimeTarget,
    resetRuntimeTargetDraft, saveRuntimeTarget, removeRuntimeTarget, clearComparison,
  } = runtime;
  const {
    currentProvider, personalTarget, setPersonalTarget, removingPack, busy, removePack,
  } = personal;

  return (
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
                clearComparison();
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
  );
}
