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
  const pending = projection.classificationReview.filter((item) => item.access === "unclassified");
  const automatic = projection.classificationReview.filter((item) => item.classifiedBy === "heuristic" && item.access !== "unclassified");
  const pendingFiles = projection.files.flatMap((file) => {
    const count = pending.filter((item) => item.files.includes(file.path)).length;
    return count > 0 ? [{ path: file.path, displayName: file.displayName, count }] : [];
  });
  const activeFile = selectedFile && pendingFiles.some((file) => file.path === selectedFile) ? selectedFile : null;
  const visiblePending = activeFile ? pending.filter((item) => item.files.includes(activeFile)) : pending;

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
      await api.protectVariables(projectId, visiblePending.map((item) => item.key));
      await onRefresh();
      onNotice(t("review.protectedAll", { count: visiblePending.length }));
    } catch (error) {
      onError(localizeError(error, locale, "review.saveError"));
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="page-stack review-page">
      <div className="section-heading review-heading">
        <div><p className="eyebrow">ACCESS REVIEW · NAMES ONLY</p><h2>{t("review.title")}</h2><p>{t("review.body")}</p></div>
        {visiblePending.length > 0 && <button className="secondary-button" disabled={busy !== null} onClick={() => void protectAll()}>{t("review.protectAll", { count: visiblePending.length })}</button>}
      </div>
      <section className="panel review-panel">
        <div className="panel-title"><h3>{t("review.needsDecision")}</h3><span>{visiblePending.length}</span></div>
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
            <div><strong>{item.key}</strong><p>{t("review.ambiguousReason")}</p><div className="review-paths">{item.files.map((file) => <button key={file} onClick={() => onOpenFile(file)}>{file}</button>)}</div></div>
            <div className="review-actions">
              {item.clientExposed && <span className="client-warning-chip">{t("review.clientVisible")}</span>}
              <button disabled={busy !== null} onClick={() => void setAccess(item.key, "protected")}>{t("row.protected")}</button>
              <button className="allow" disabled={busy !== null} onClick={() => void setAccess(item.key, "read-write")}>{t("row.readWrite")}</button>
            </div>
          </article>
        ))}
      </section>
      <section className="panel review-panel automatic-panel">
        <div className="panel-title"><h3>{t("review.automatic")}</h3><span>{automatic.length}</span></div>
        <p className="panel-intro">{t("review.automaticBody")}</p>
        <div className="automatic-policies">
          {automatic.map((item) => <span className={item.access} key={item.key}><strong>{item.key}</strong><small>{item.access === "protected" ? t("row.protected") : t("row.readWrite")}</small></span>)}
        </div>
      </section>
      <p className="audit-footnote">{t("review.independent")}</p>
    </section>
  );
}
