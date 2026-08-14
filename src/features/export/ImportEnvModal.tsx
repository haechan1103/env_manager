import { useEffect, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { TeamImportPlanProjection } from "../../lib/types";

interface Props {
  projectId: string;
  onApplied: () => Promise<void>;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function ImportEnvModal({ projectId, onApplied, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [passphrase, setPassphrase] = useState("");
  const [plan, setPlan] = useState<TeamImportPlanProjection | null>(null);
  const [sharedConflicts, setSharedConflicts] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => () => {
    if (plan) void api.discardTeamImport(projectId, plan.planId);
  }, [plan, projectId]);

  const choose = async () => {
    if (passphrase.length < 10) return;
    setBusy(true);
    try {
      const result = await api.planTeamImport(projectId, passphrase, locale);
      setPassphrase("");
      if (result) setPlan(result);
    } catch (error) {
      setPassphrase("");
      onError(localizeError(error, locale, "import.error"));
    } finally {
      setBusy(false);
    }
  };

  const toggleConflict = (id: string, linkId: string | null, checked: boolean) => {
    if (!plan) return;
    const ids = linkId
      ? plan.preview.files.flatMap((file) => file.occurrences)
          .filter((item) => item.linkId === linkId && item.state === "conflict")
          .map((item) => item.id)
      : [id];
    setSharedConflicts((previous) => {
      const next = new Set(previous);
      for (const item of ids) checked ? next.add(item) : next.delete(item);
      return next;
    });
  };

  const apply = async () => {
    if (!plan) return;
    setBusy(true);
    try {
      const result = await api.applyTeamImport(projectId, plan.planId, [...sharedConflicts]);
      setPlan(null);
      await onApplied();
      onNotice(t("import.done", {
        added: result.addedCount,
        updated: result.updatedCount,
        kept: result.keptLocalCount,
        unchanged: result.unchangedCount,
      }));
      onClose();
    } catch (error) {
      onError(localizeError(error, locale, "import.error"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title={t("import.title")} description={t("import.description")} onClose={onClose}>
      {!plan ? (
        <>
          <div className="modal-form import-password-field">
            <label><span>{t("import.passphrase")}</span><input type="password" autoComplete="off" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} /></label>
            <p className="field-help">{t("import.passphraseHelp")}</p>
          </div>
          <div className="export-warning"><strong>{t("import.warningTitle")}</strong><p>{t("import.warningBody")}</p></div>
          <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={busy || passphrase.length < 10} onClick={() => void choose()}>{busy ? t("import.opening") : t("import.choose")}</button></div>
        </>
      ) : (
        <>
          <div className="import-summary" aria-live="polite">
            <span><strong>{plan.preview.newCount}</strong>{t("import.new")}</span>
            <span><strong>{plan.preview.unchangedCount}</strong>{t("import.unchanged")}</span>
            <span className={plan.preview.conflictCount > 0 ? "conflict" : undefined}><strong>{plan.preview.conflictCount}</strong>{t("import.conflict")}</span>
          </div>
          <div className="import-file-list">
            {plan.preview.files.map((file) => (
              <section className="import-file-group" key={file.path}>
                <header><code>{file.path}</code><small>{file.occurrences.length}</small></header>
                {file.occurrences.map((occurrence) => (
                  <div className="import-occurrence" key={occurrence.id}>
                    <code>{occurrence.key}</code>
                    {occurrence.state === "conflict" ? (
                      <label className="conflict-choice">
                        <span>{t("import.keepLocal")}</span>
                        <input aria-label={t("import.useShared")} type="checkbox" checked={sharedConflicts.has(occurrence.id)} onChange={(event) => toggleConflict(occurrence.id, occurrence.linkId, event.target.checked)} />
                        <span>{t("import.useShared")}</span>
                      </label>
                    ) : (
                      <span className={`import-state ${occurrence.state}`}>{t(`import.state.${occurrence.state}`)}</span>
                    )}
                    {occurrence.linkId && <small className="linked-share-note">{t("import.linkedTogether")}</small>}
                  </div>
                ))}
              </section>
            ))}
          </div>
          {plan.preview.conflictCount > 0 && <div className="export-warning import-conflict-warning"><strong>{t("import.conflictTitle")}</strong><p>{t("import.conflictBody", { count: plan.preview.conflictCount })}</p></div>}
          <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={busy} onClick={() => void apply()}>{busy ? t("import.applying") : t("import.apply")}</button></div>
        </>
      )}
    </Modal>
  );
}
