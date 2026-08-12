import { useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";

interface Props {
  projectId: string;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function ExportEnvModal({ projectId, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [mode, setMode] = useState<"plain" | "encrypted">("plain");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const valid = mode === "plain" || (passphrase.length >= 10 && passphrase === confirm);

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    try {
      const result = await api.exportEnvFiles(projectId, mode === "encrypted" ? passphrase : null, locale);
      if (!result.cancelled) {
        onNotice(t(result.encrypted ? "export.encryptedDone" : "export.plainDone", { count: result.fileCount }));
        onClose();
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
    <Modal title={t("export.title")} description={t("export.description")} onClose={onClose}>
      <div className="export-options">
        <button className={mode === "plain" ? "export-option selected" : "export-option"} onClick={() => setMode("plain")}>
          <span className="export-option-mark">ZIP</span><span><strong>{t("export.plain")}</strong><small>{t("export.plainBody")}</small></span>
        </button>
        <button className={mode === "encrypted" ? "export-option selected" : "export-option"} onClick={() => setMode("encrypted")}>
          <span className="export-option-mark secure">AGE</span><span><strong>{t("export.encrypted")}</strong><small>{t("export.encryptedBody")}</small></span>
        </button>
      </div>
      {mode === "encrypted" && (
        <div className="modal-form export-password-fields">
          <label><span>{t("export.passphrase")}</span><input type="password" autoComplete="new-password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} /></label>
          <label><span>{t("export.confirmPassphrase")}</span><input type="password" autoComplete="new-password" value={confirm} onChange={(event) => setConfirm(event.target.value)} /></label>
          <p className={confirm && passphrase !== confirm ? "field-error" : "field-help"}>{confirm && passphrase !== confirm ? t("export.mismatch") : t("export.passphraseHelp")}</p>
        </div>
      )}
      <div className="export-warning"><strong>{t("export.warningTitle")}</strong><p>{t(mode === "plain" ? "export.plainWarning" : "export.encryptedWarning")}</p></div>
      <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={!valid || busy} onClick={() => void submit()}>{busy ? t("export.exporting") : t("export.action")}</button></div>
    </Modal>
  );
}
