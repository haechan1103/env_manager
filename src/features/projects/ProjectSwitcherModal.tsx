import { Modal } from "../../components/Modal";
import { useI18n } from "../../i18n";
import type { ProjectSummary } from "../../lib/types";

interface Props {
  projects: ProjectSummary[];
  selectedProjectId: string | null;
  onClose: () => void;
  onRegister: () => void;
  onSelectProject: (projectId: string) => void;
}

export function ProjectSwitcherModal({
  projects,
  selectedProjectId,
  onClose,
  onRegister,
  onSelectProject,
}: Props) {
  const { t } = useI18n();

  return (
    <Modal
      className="project-switcher-modal"
      title={t("projectSwitcher.title")}
      description={t("projectSwitcher.description")}
      onClose={onClose}
    >
      <nav className="project-switcher-list" aria-label={t("sidebar.registeredProjects")}>
        {projects.map((project) => {
          const isCurrent = project.id === selectedProjectId;
          return (
            <button
              type="button"
              className={isCurrent ? "project-switcher-option current" : "project-switcher-option"}
              aria-current={isCurrent ? "true" : undefined}
              key={project.id}
              onClick={() => {
                onSelectProject(project.id);
                onClose();
              }}
            >
              <span className="project-glyph" aria-hidden="true">
                {project.name.slice(0, 1).toUpperCase()}
              </span>
              <span className="project-switcher-copy">
                <strong>{project.name}</strong>
                <small>{project.displayPath}</small>
              </span>
              {isCurrent && <span className="current-project-badge">{t("common.current")}</span>}
            </button>
          );
        })}
        {projects.length === 0 && (
          <div className="project-switcher-empty">{t("sidebar.noProjects")}</div>
        )}
      </nav>
      <div className="modal-actions project-switcher-actions">
        <button
          type="button"
          className="primary-button"
          onClick={() => {
            onClose();
            onRegister();
          }}
        >
          {t("projectSwitcher.add")}
        </button>
      </div>
    </Modal>
  );
}
