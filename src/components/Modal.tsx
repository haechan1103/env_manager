import type { ReactNode } from "react";
import { useI18n } from "../i18n";

interface Props {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
}

export function Modal({ title, description, children, onClose }: Props) {
  const { t } = useI18n();
  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <h2 id="modal-title">{title}</h2>
            {description && <p>{description}</p>}
          </div>
          <button className="icon-button" aria-label={t("modal.close")} onClick={onClose}>×</button>
        </header>
        {children}
      </section>
    </div>
  );
}
