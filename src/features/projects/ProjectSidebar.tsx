import { useState } from "react";

import { RenameModal } from "../../components/RenameModal";
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
  const [renameTarget, setRenameTarget] = useState<
    | { kind: "project"; id: string; name: string }
    | { kind: "file"; projectId: string; path: string; name: string }
    | null
  >(null);
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          <img src="/brand/env-manager-logo-v1.png" alt="" />
        </span>
        <span>Env Manager</span>
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
            onDoubleClick={() => setRenameTarget({ kind: "project", id: project.id, name: project.name })}
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
                if (selectedProjectId) {
                  setRenameTarget({ kind: "file", projectId: selectedProjectId, path: file.path, name: file.displayName });
                }
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
        <AppUpdater />
      </div>
      {renameTarget && (
        <RenameModal
          title={t(renameTarget.kind === "project" ? "sidebar.projectNamePrompt" : "sidebar.fileNamePrompt")}
          currentName={renameTarget.name}
          onClose={() => setRenameTarget(null)}
          onRename={(name) => {
            if (renameTarget.kind === "project") onRenameProject(renameTarget.id, name);
            else onRenameFile(renameTarget.projectId, renameTarget.path, name);
          }}
        />
      )}
    </aside>
  );
}
