import type { ProjectProjection, ProjectSummary } from "../../lib/types";
import { supportedLocales, useI18n, type Locale } from "../../i18n";
import { AppUpdater } from "../updater/AppUpdater";

interface View {
  kind: "overview" | "file" | "integrations" | "activity" | "review";
  path?: string;
}

interface Props {
  projects: ProjectSummary[];
  selectedProjectId: string | null;
  projection: ProjectProjection | null;
  view: View;
  onSelectProject: (projectId: string) => void;
  onSelectView: (
    view: { kind: "overview" } | { kind: "file"; path: string } | { kind: "integrations" } | { kind: "activity" } | { kind: "review" },
  ) => void;
  onRegister: () => void;
  onRenameProject: (projectId: string, name: string) => void;
  onRenameFile: (projectId: string, path: string, name: string) => void;
}

export function ProjectSidebar({
  projects,
  selectedProjectId,
  projection,
  view,
  onSelectProject,
  onSelectView,
  onRegister,
  onRenameProject,
  onRenameFile,
}: Props) {
  const { locale, setLocale, t } = useI18n();
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">E</span>
        <span>Env Manager</span>
        <AppUpdater />
      </div>

      <div className="sidebar-section-title">
        <span>PROJECTS</span>
        <button aria-label={t("sidebar.registerProject")} onClick={onRegister}>+</button>
      </div>

      <nav className="project-list" aria-label={t("sidebar.registeredProjects")}>
        {projects.map((project) => (
          <button
            key={project.id}
            className={project.id === selectedProjectId ? "project-item selected" : "project-item"}
            onClick={() => onSelectProject(project.id)}
            onDoubleClick={() => {
              const name = window.prompt(t("sidebar.projectNamePrompt"), project.name)?.trim();
              if (name && name !== project.name) onRenameProject(project.id, name);
            }}
            title={t("sidebar.renameHint")}
          >
            <span className="project-glyph">{project.name.slice(0, 1).toUpperCase()}</span>
            <span className="project-name">{project.name}</span>
          </button>
        ))}
        {projects.length === 0 && (
          <div className="sidebar-empty">
            <span>—</span>
            {t("sidebar.noProjects")}
          </div>
        )}
      </nav>

      {projection && (
        <nav className="file-navigation" aria-label={t("sidebar.projectViews")}>
          <button
            className={view.kind === "overview" ? "nav-item active" : "nav-item"}
            onClick={() => onSelectView({ kind: "overview" })}
          >
            <span>⌁</span> Overview
            {(projection.issueCount > 0 || projection.unclassifiedCount > 0) && (
              <b>{projection.issueCount + projection.unclassifiedCount}</b>
            )}
          </button>
          <button
            className={view.kind === "review" ? "nav-item active" : "nav-item"}
            onClick={() => onSelectView({ kind: "review" })}
          >
            <span>◇</span> {t("sidebar.review")}
            {projection.unclassifiedCount > 0 && <b>{projection.unclassifiedCount}</b>}
          </button>
          <button
            className={view.kind === "activity" ? "nav-item active" : "nav-item"}
            onClick={() => onSelectView({ kind: "activity" })}
          >
            <span>◷</span> {t("sidebar.activity")}
          </button>
          <p className="file-label">ENV FILES</p>
          {projection.files.map((file) => (
            <button
              key={file.path}
              className={view.kind === "file" && view.path === file.path ? "nav-item active" : "nav-item"}
              onClick={() => onSelectView({ kind: "file", path: file.path })}
              onDoubleClick={() => {
                const name = window.prompt(t("sidebar.fileNamePrompt"), file.displayName)?.trim();
                if (name && name !== file.displayName && selectedProjectId) onRenameFile(selectedProjectId, file.path, name);
              }}
              title={`${file.displayName}\n${file.path}\n${t("sidebar.renameHint")}`}
            >
              <span className="file-dot" />
              <span className="sidebar-file-copy">
                <span className="truncate">{file.displayName}</span>
                {file.displayName !== file.path && <small className="truncate">{file.path}</small>}
              </span>
              {file.warnings.length > 0 && <b>!</b>}
            </button>
          ))}
        </nav>
      )}

      <div className="sidebar-footer">
        <nav className="agent-navigation footer-agent-navigation" aria-label={t("sidebar.aiTools")}>
          <button
            className={view.kind === "integrations" ? "nav-item active" : "nav-item"}
            onClick={() => onSelectView({ kind: "integrations" })}
          >
            <span>◇</span>
            <span className="agent-nav-label">{t("sidebar.aiConnections")}</span>
          </button>
        </nav>
        <label className="language-control">
          <span>{t("language.label")}</span>
          <select
            value={locale}
            aria-label={t("language.label")}
            onChange={(event) => setLocale(event.target.value as Locale)}
          >
            {supportedLocales.map((option) => (
              <option value={option.code} key={option.code}>{option.label}</option>
            ))}
          </select>
        </label>
        <div className="local-status">
          <span className="status-dot" />
          <span>{t("sidebar.localOnly")}</span>
        </div>
      </div>
    </aside>
  );
}
