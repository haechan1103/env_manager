import { useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { ExportOccurrence, ProjectProjection } from "../../lib/types";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
  channelId?: string;
}

const occurrenceId = (file: string, key: string) => `${file}\0${key}`;

export function ExportEnvModal({ projectId, projection, onClose, onError, onNotice, channelId }: Props) {
  const { locale, t } = useI18n();
  const [mode, setMode] = useState<"plain" | "encrypted">("encrypted");
  const [scope, setScope] = useState<"all" | "selected">("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const occurrences = useMemo(
    () => projection.files.flatMap((file) => file.groups.flatMap((group) =>
      group.variables.map((variable) => ({ file: file.path, variable })))),
    [projection],
  );
  const valid = (scope === "all" || selected.size > 0)
    && (mode === "plain" || (passphrase.length >= 10 && passphrase === confirm));

  const linkedIds = (file: string, key: string) => {
    const current = occurrences.find((item) => item.file === file && item.variable.key === key);
    if (!current?.variable.linkId) return [occurrenceId(file, key)];
    return current.variable.linkedFiles.map((path) => occurrenceId(path, key));
  };

  const toggleOccurrence = (file: string, key: string, checked: boolean) => {
    setSelected((previous) => {
      const next = new Set(previous);
      for (const id of linkedIds(file, key)) checked ? next.add(id) : next.delete(id);
      return next;
    });
  };

  const toggleFile = (file: string, checked: boolean) => {
    const fileOccurrences = occurrences.filter((item) => item.file === file);
    setSelected((previous) => {
      const next = new Set(previous);
      for (const item of fileOccurrences) {
        for (const id of linkedIds(file, item.variable.key)) checked ? next.add(id) : next.delete(id);
      }
      return next;
    });
  };

  const submit = async () => {
    if (!valid) return;
    const selection: ExportOccurrence[] | null = scope === "all"
      ? null
      : occurrences
          .filter((item) => selected.has(occurrenceId(item.file, item.variable.key)))
          .map((item) => ({ file: item.file, key: item.variable.key }));
    setBusy(true);
    try {
      if (channelId) {
        const result = await api.publishTeamChannel(projectId, channelId, passphrase, selection);
        onNotice(t("teamChannel.published", { count: result.fileCount }));
        onClose();
      } else {
        const result = await api.exportEnvFiles(
          projectId,
          mode === "encrypted" ? passphrase : null,
          selection,
          locale,
        );
        if (!result.cancelled) {
          onNotice(t(result.encrypted ? "export.encryptedDone" : "export.plainDone", { count: result.fileCount }));
          onClose();
        }
      }
    } catch (error) {
      onError(localizeError(error, locale, "export.error"));
    } finally {
      setBusy(false);
      setPassphrase("");
      setConfirm("");
    }
  };

  return (
    <Modal title={t(channelId ? "teamChannel.publishTitle" : "export.title")} description={t(channelId ? "teamChannel.publishDescription" : "export.description")} onClose={onClose}>
      {!channelId && <div className="export-options">
        <button className={mode === "encrypted" ? "export-option selected" : "export-option"} onClick={() => setMode("encrypted")}>
          <span className="export-option-mark secure">AGE</span><span><strong>{t("export.encrypted")}</strong><small>{t("export.encryptedBody")}</small></span>
        </button>
        <button className={mode === "plain" ? "export-option selected" : "export-option"} onClick={() => setMode("plain")}>
          <span className="export-option-mark">ZIP</span><span><strong>{t("export.plain")}</strong><small>{t("export.plainBody")}</small></span>
        </button>
      </div>}
      <div className="share-scope-options">
        <label><input type="radio" checked={scope === "all"} onChange={() => setScope("all")} /><span><strong>{t("export.scopeAll")}</strong><small>{t("export.scopeAllBody")}</small></span></label>
        <label><input type="radio" checked={scope === "selected"} onChange={() => setScope("selected")} /><span><strong>{t("export.scopeSelected")}</strong><small>{t("export.scopeSelectedBody")}</small></span></label>
      </div>
      {scope === "selected" && (
        <div className="share-selection-list">
          {projection.files.map((file) => {
            const variables = file.groups.flatMap((group) => group.variables);
            const checkedCount = variables.filter((item) => selected.has(occurrenceId(file.path, item.key))).length;
            return (
              <section key={file.path}>
                <label className="share-file-select">
                  <input type="checkbox" checked={variables.length > 0 && checkedCount === variables.length} onChange={(event) => toggleFile(file.path, event.target.checked)} />
                  <code>{file.path}</code><small>{checkedCount}/{variables.length}</small>
                </label>
                {variables.map((variable) => (
                  <label className="share-variable-select" key={`${file.path}:${variable.key}`}>
                    <input type="checkbox" checked={selected.has(occurrenceId(file.path, variable.key))} onChange={(event) => toggleOccurrence(file.path, variable.key, event.target.checked)} />
                    <code>{variable.key}</code>
                    {variable.linkedCount > 1 && <small>{t("export.linkedSelection", { count: variable.linkedCount })}</small>}
                  </label>
                ))}
              </section>
            );
          })}
        </div>
      )}
      {(channelId || mode === "encrypted") && (
        <div className="modal-form export-password-fields">
          <label><span>{t("export.passphrase")}</span><input type="password" autoComplete="new-password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} /></label>
          <label><span>{t("export.confirmPassphrase")}</span><input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} /></label>
          <p className={confirm && passphrase !== confirm ? "field-error" : "field-help"}>{confirm && passphrase !== confirm ? t("export.mismatch") : t("export.passphraseHelp")}</p>
        </div>
      )}
      <div className="export-warning"><strong>{t("export.warningTitle")}</strong><p>{t(channelId ? "teamChannel.publishWarning" : mode === "plain" ? "export.plainWarning" : "export.encryptedWarning")}</p></div>
      <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={!valid || busy} onClick={() => void submit()}>{busy ? t(channelId ? "teamChannel.publishing" : "export.exporting") : t(channelId ? "teamChannel.publish" : "export.action")}</button></div>
    </Modal>
  );
}
