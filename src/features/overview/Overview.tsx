import type { ProjectProjection } from "../../lib/types";
import { useI18n } from "../../i18n";

interface Props {
  projection: ProjectProjection;
  onOpenFile: (path: string) => void;
}

export function Overview({ projection, onOpenFile }: Props) {
  const { t } = useI18n();
  const variables = projection.files.flatMap((file) =>
    file.groups.flatMap((group) => group.variables.map((variable) => ({ ...variable, file: file.path }))),
  );
  const empty = variables.filter((variable) => variable.valueState === "empty");
  const linked = variables.filter((variable) => variable.linkedCount > 0).length;
  const protectedCount = variables.filter((variable) => variable.codexAccess === "protected").length;

  return (
    <section className="page-stack">
      <div className="section-heading">
        <div>
          <p className="eyebrow">PROJECT HEALTH</p>
          <h2>{t("overview.heading")}</h2>
        </div>
      </div>

      <div className="stats-grid">
        <Stat label={t("overview.managedFiles")} value={projection.files.length} detail={t("overview.insideProject")} />
        <Stat label={t("overview.variables")} value={variables.length} detail={t("overview.protectedCount", { count: protectedCount })} />
        <Stat label={t("overview.linkedOccurrences")} value={linked} detail={t("overview.explicitLinks")} />
        <Stat
          label={t("overview.actionRequired")}
          value={empty.length + projection.unclassifiedCount + projection.issueCount}
          detail={t("overview.actionTypes")}
          tone={empty.length + projection.unclassifiedCount + projection.issueCount > 0 ? "warn" : "good"}
        />
      </div>

      <div className="overview-grid">
        <section className="panel">
          <div className="panel-title">
            <h3>Action inbox</h3>
            <span>{empty.length + projection.unclassifiedCount + projection.issueCount}</span>
          </div>
          {empty.length === 0 && projection.unclassifiedCount === 0 && projection.issueCount === 0 ? (
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
                <div className="issue-row">
                  <span className="issue-icon violet">?</span>
                  <span><strong>{t("overview.unclassified")}</strong><small>{t("overview.variablesToReview", { count: projection.unclassifiedCount })}</small></span>
                </div>
              )}
              {projection.issueCount > 0 && (
                <div className="issue-row">
                  <span className="issue-icon red">!</span>
                  <span><strong>{t("overview.parseWarnings")}</strong><small>{t("overview.warningsPreserved", { count: projection.issueCount })}</small></span>
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

function Stat({ label, value, detail, tone }: { label: string; value: number; detail: string; tone?: "warn" | "good" }) {
  return (
    <article className={`stat-card ${tone ?? ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
