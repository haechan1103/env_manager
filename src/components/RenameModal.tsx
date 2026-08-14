import { useState } from "react";

import { useI18n } from "../i18n";
import { Modal } from "./Modal";

interface Props {
  title: string;
  currentName: string;
  onClose: () => void;
  onRename: (name: string) => void;
}

export function RenameModal({ title, currentName, onClose, onRename }: Props) {
  const { t } = useI18n();
  const [name, setName] = useState(currentName);
  const trimmedName = name.trim();
  const canSave = trimmedName.length > 0 && trimmedName !== currentName;

  return (
    <Modal title={title} onClose={onClose}>
      <form
        className="modal-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!canSave) return;
          onRename(trimmedName);
          onClose();
        }}
      >
        <label>
          {t("common.newName")}
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onFocus={(event) => event.currentTarget.select()}
          />
        </label>
        <div className="modal-actions">
          <button type="button" className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
          <button className="primary-button" disabled={!canSave}>{t("common.save")}</button>
        </div>
      </form>
    </Modal>
  );
}
