import { useState } from "react";

import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { CodexAccess, ProjectProjection } from "../../lib/types";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onRefresh: () => Promise<void>;
  onOpenFile: (path: string) => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function ClassificationReview({ projectId, projection, onRefresh, onOpenFile, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [busy, setBusy] = useState<string | null>(null);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [showAllUnclassified, setShowAllUnclassified] = useState(false);
  const plainUnclassified = projection.classificationReview.filter(
    (item) => item.access === "unclassified" && item.reviewReasons.length === 0,
  );
  const pending = projection.classificationReview.filter(
    (item) => item.reviewReasons.length > 0 || (showAllUnclassified && item.access === "unclassified"),
  );
  const pendingFiles = projection.files.flatMap((file) => {
    const count = pending.filter((item) => item.files.includes(file.path)).length;
    return count > 0 ? [{ path: file.path, displayName: file.displayName, count }] : [];
  });
  const activeFile = selectedFile && pendingFiles.some((file) => file.path === selectedFile) ? selectedFile : null;
  const visiblePending = activeFile ? pending.filter((item) => item.files.includes(activeFile)) : pending;
  const protectable = visiblePending.filter((item) => item.access === "unclassified");

  const setAccess = async (key: string, access: CodexAccess) => {
    setBusy(key);
    try {
      await api.setCodexAccess(projectId, key, access, access === "read-write");
      await onRefresh();
      onNotice(t("review.saved", { key }));
    } catch (error) {
      onError(localizeError(error, locale, "review.saveError"));
    } finally {
      setBusy(null);
    }
  };

  const protectAll = async () => {
    setBusy("*");
    try {
      await api.protectVariables(projectId, protectable.map((item) => item.key));
      await onRefresh();
      onNotice(t("review.protectedAll", { count: protectable.length }));
    } catch (error) {
      onError(localizeError(error, locale, "review.saveError"));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="page-stack review-page">
      <div className="section-heading review-heading">
        <div><h2>{t("review.title")}</h2><p>{t("review.body")}</p></div>
        <div className="review-heading-actions">
          {plainUnclassified.length > 0 && (
            <button
              className="quiet-button"
              aria-controls="classification-review-list"
              aria-expanded={showAllUnclassified}
              onClick={() => setShowAllUnclassified((current) => !current)}
            >
              {showAllUnclassified
                ? t("review.hideUnclassified")
                : t("review.showUnclassified", { count: plainUnclassified.length })}
            </button>
          )}
          {protectable.length > 0 && <button className="secondary-button" disabled={busy !== null} onClick={() => void protectAll()}>{t("review.protectAll", { count: protectable.length })}</button>}
        </div>
      </div>
      <section className="panel review-panel" id="classification-review-list">
        <div className="panel-title"><h3>{t("review.needsDecision")}</h3><span>{visiblePending.length}</span></div>
        {visiblePending.some((item) => item.access === "unclassified") && <p className="review-list-help">{t("review.ambiguousReason")}</p>}
        {pendingFiles.length > 1 && (
          <div className="review-file-tabs" role="tablist" aria-label={t("review.fileFilter")}>
            <button className={activeFile === null ? "active" : ""} role="tab" aria-label={t("review.fileTab", { name: t("review.allFiles"), count: pending.length })} aria-selected={activeFile === null} onClick={() => setSelectedFile(null)}>
              <span>{t("review.allFiles")}</span><b>{pending.length}</b>
            </button>
            {pendingFiles.map((file) => (
              <button className={activeFile === file.path ? "active" : ""} role="tab" aria-label={t("review.fileTab", { name: file.displayName, count: file.count })} aria-selected={activeFile === file.path} title={file.path} key={file.path} onClick={() => setSelectedFile(file.path)}>
                <span>{file.displayName}</span><b>{file.count}</b>
              </button>
            ))}
          </div>
        )}
        {visiblePending.length === 0 ? <div className="empty-inline"><span>✓</span><p>{t("review.empty")}</p></div> : visiblePending.map((item) => (
          <article className="review-item" key={item.key}>
            <div><strong>{item.key}</strong><div className="review-paths">{item.files.map((file) => <button key={file} onClick={() => onOpenFile(file)}>{file}</button>)}</div></div>
            <div className="review-actions">
              {item.reviewReasons.includes("client-exposure-conflict") && <span className="client-warning-chip">{t("review.clientVisible")}</span>}
              {item.reviewReasons.includes("agent-access-request") && <span className="agent-request-chip">{t("review.agentRequested")}</span>}
              <button disabled={busy !== null} onClick={() => void setAccess(item.key, "protected")}>{t("row.protected")}</button>
              <button className="allow" disabled={busy !== null} onClick={() => void setAccess(item.key, "read-write")}>{t("row.readWrite")}</button>
            </div>
          </article>
        ))}
      </section>
      <p className="audit-footnote">{t("review.independent")}</p>
    </section>
  );
}
