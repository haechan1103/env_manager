import { useState } from "react";

import type { GitSafetyProjection, ProjectProjection } from "../../lib/types";
import { useI18n } from "../../i18n";

interface Props {
  projection: ProjectProjection;
  onOpenFile: (path: string) => void;
  onApplyGitignoreGuard: () => Promise<void>;
  onOpenReview: () => void;
}

export function Overview({ projection, onOpenFile, onApplyGitignoreGuard, onOpenReview }: Props) {
  const { t } = useI18n();
  const variables = projection.files.flatMap((file) =>
    file.groups.flatMap((group) => group.variables.map((variable) => ({ ...variable, file: file.path }))),
  );
  const empty = variables.filter((variable) => variable.valueState === "empty");
  const linked = variables.filter((variable) => variable.linkedCount > 0).length;
  const protectedCount = variables.filter((variable) => variable.codexAccess === "protected").length;
  const accessByKey = new Map(variables.map((variable) => [variable.key, variable.codexAccess]));
  const accessPolicies = [...accessByKey.values()];
  const blockedPolicyCount = accessPolicies.filter((access) => access !== "read-write").length;
  const allowedPolicyCount = accessPolicies.filter((access) => access === "read-write").length;
  const gitAttentionCount = projection.gitSafety.state === "needs-attention" ? 1 : 0;
  const actionCount = empty.length + projection.unclassifiedCount + projection.issueCount + gitAttentionCount + projection.clientExposureCount;

  return (
    <section className="page-stack">
      <div className="section-heading">
        <div>
          <h2>{t("overview.heading")}</h2>
        </div>
      </div>

      <div className="stats-grid">
        <Stat label={t("overview.managedFiles")} value={projection.files.length} detail={t("overview.insideProject")} />
        <Stat label={t("overview.variables")} value={variables.length} detail={t("overview.protectedCount", { count: protectedCount })} />
        <Stat label={t("overview.linkedOccurrences")} value={linked} detail={t("overview.explicitLinks")} />
        <Stat
          label={t("overview.actionRequired")}
          value={actionCount}
          detail={t("overview.actionTypes")}
          tone={actionCount > 0 ? "warn" : "good"}
        />
      </div>

      <div className="overview-grid">
        <section className="panel">
          <div className="panel-title">
            <h3>{t("overview.actionInbox")}</h3>
            <span>{actionCount}</span>
          </div>
          {actionCount === 0 ? (
            <div className="empty-inline">
              <span>✓</span>
              <p>{t("overview.noIssues")}</p>
            </div>
          ) : (
            <div className="issue-list">
              {empty.map((variable) => (
                <button key={`${variable.file}:${variable.key}`} onClick={() => onOpenFile(variable.file)}>
                  <span className="issue-icon amber">○</span>
                  <span><strong>{variable.key}</strong><small>{variable.file} · {t("overview.valueRequired")}</small></span>
                  <span>→</span>
                </button>
              ))}
              {projection.unclassifiedCount > 0 && (
                <button onClick={onOpenReview}>
                  <span className="issue-icon violet">?</span>
                  <span><strong>{t("overview.unclassified")}</strong><small>{t("overview.variablesToReview", { count: projection.unclassifiedCount })}</small></span>
                  <span>→</span>
                </button>
              )}
              {variables.filter((variable) => variable.clientExposure).map((variable) => (
                <button key={`exposure:${variable.file}:${variable.key}`} onClick={() => onOpenFile(variable.file)}>
                  <span className="issue-icon red">!</span>
                  <span><strong>{variable.key}</strong><small>{variable.file} · {t("overview.clientExposure")}</small></span>
                  <span>→</span>
                </button>
              ))}
              {projection.issueCount > 0 && (
                <div className="issue-row">
                  <span className="issue-icon red">!</span>
                  <span><strong>{t("overview.parseWarnings")}</strong><small>{t("overview.warningsPreserved", { count: projection.issueCount })}</small></span>
                </div>
              )}
              {gitAttentionCount > 0 && (
                <div className="issue-row">
                  <span className="issue-icon red">G</span>
                  <span><strong>{t("overview.gitLeakRisk")}</strong><small>{t("overview.gitLeakRiskBody")}</small></span>
                </div>
              )}
            </div>
          )}
        </section>

        <section className="panel">
          <div className="panel-title"><h3>{t("overview.managedFiles")}</h3><span>{projection.files.length}</span></div>
          <div className="managed-files">
            {projection.files.map((file) => {
              const count = file.groups.reduce((total, group) => total + group.variables.length, 0);
              return (
                <button key={file.path} onClick={() => onOpenFile(file.path)}>
                  <span className="file-icon">ENV</span>
                  <span><strong>{file.path}</strong><small>{t("overview.fileSummary", { variables: count, groups: file.groups.length })}</small></span>
                  <span>→</span>
                </button>
              );
            })}
          </div>
        </section>
      </div>

      <div className="protection-grid">
        <GitSafetyCard
          safety={projection.gitSafety}
          onApply={onApplyGitignoreGuard}
        />
        <section className="protection-card ai-protection-card">
          <header>
            <span className="protection-mark ai" aria-hidden="true">AI</span>
            <div>
              <p className="protection-label">{t("overview.aiAccessTitle")}</p>
              <h3>{t("overview.aiAccessProtected", { count: blockedPolicyCount })}</h3>
            </div>
            <span className="protection-state good">{t("overview.policyActive")}</span>
          </header>
          <p>{t("overview.aiAccessBody")}</p>
          <dl className="protection-counts">
            <div>
              <dt>{t("overview.blockedValues")}</dt>
              <dd>{blockedPolicyCount}</dd>
            </div>
            <div>
              <dt>{t("overview.allowedValues")}</dt>
              <dd>{allowedPolicyCount}</dd>
            </div>
          </dl>
          <small>{t("overview.aiBoundaryNote")}</small>
        </section>
      </div>

      <div className="security-note">
        <span>⌾</span>
        <div>
          <strong>{t("overview.securityTitle")}</strong>
          <p>{t("overview.securityBody")}</p>
        </div>
      </div>
    </section>
  );
}

function GitSafetyCard({
  safety,
  onApply,
}: {
  safety: GitSafetyProjection;
  onApply: () => Promise<void>;
}) {
  const { t } = useI18n();
  const [applying, setApplying] = useState(false);
  const needsAttention = safety.state === "needs-attention";
  const apply = async () => {
    setApplying(true);
    try {
      await onApply();
    } catch {
      // The application shell presents the localized failure toast.
    } finally {
      setApplying(false);
    }
  };

  const title = {
    protected: t("overview.gitProtected"),
    "needs-attention": t("overview.gitAttention"),
    "not-repository": t("overview.gitNotRepository"),
    unavailable: t("overview.gitUnavailable"),
  }[safety.state];
  const body = {
    protected: t("overview.gitProtectedBody", { count: safety.ignoredFiles.length }),
    "needs-attention": t("overview.gitAttentionBody"),
    "not-repository": t("overview.gitNotRepositoryBody"),
    unavailable: t("overview.gitUnavailableBody"),
  }[safety.state];

  return (
    <section className={`protection-card git-protection-card ${needsAttention ? "attention" : ""}`}>
      <header>
        <span className="protection-mark git" aria-hidden="true">G</span>
        <div>
          <p className="protection-label">{t("overview.gitSafetyTitle")}</p>
          <h3>{title}</h3>
        </div>
        <span className={`protection-state ${safety.state === "protected" ? "good" : needsAttention ? "warn" : "neutral"}`}>
          {safety.state === "protected"
            ? t("overview.protectedStatus")
            : needsAttention
              ? t("overview.reviewStatus")
              : t("overview.infoStatus")}
        </span>
      </header>
      <p>{body}</p>

      {needsAttention && (
        <div className="git-risk-list">
          {safety.missingIgnoreFiles.length > 0 && (
            <div>
              <strong>{t("overview.notIgnored", { count: safety.missingIgnoreFiles.length })}</strong>
              <div>{safety.missingIgnoreFiles.map((path) => <code key={path}>{path}</code>)}</div>
            </div>
          )}
          {safety.trackedFiles.length > 0 && (
            <div className="tracked-risk">
              <strong>{t("overview.alreadyTracked", { count: safety.trackedFiles.length })}</strong>
              <div>{safety.trackedFiles.map((path) => <code key={path}>{path}</code>)}</div>
              <small>{t("overview.trackedHelp")}</small>
            </div>
          )}
          {safety.historyFiles.length > 0 && (
            <div className="tracked-risk">
              <strong>{t("overview.inLocalHistory", { count: safety.historyFiles.length })}</strong>
              <div>{safety.historyFiles.map((path) => <code key={path}>{path}</code>)}</div>
              <small>{t("overview.historyHelp")}</small>
            </div>
          )}
          {safety.remoteHistoryFiles.length > 0 && (
            <div className="tracked-risk critical">
              <strong>{t("overview.inRemoteHistory", { count: safety.remoteHistoryFiles.length })}</strong>
              <div>{safety.remoteHistoryFiles.map((path) => <code key={path}>{path}</code>)}</div>
              <small>{t("overview.remoteHistoryHelp")}</small>
            </div>
          )}
        </div>
      )}

      {safety.missingIgnoreFiles.length > 0 && (
        <button className="secondary-button protection-action" disabled={applying} onClick={() => void apply()}>
          {applying ? t("overview.applyingGitignore") : t("overview.applyGitignore")}
        </button>
      )}
    </section>
  );
}

function Stat({ label, value, detail, tone }: { label: string; value: number; detail: string; tone?: "warn" | "good" }) {
  return (
    <article className={`stat-card ${tone ?? ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
