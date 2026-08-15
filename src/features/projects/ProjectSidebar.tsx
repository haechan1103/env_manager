import { useState } from "react";

import { RenameModal } from "../../components/RenameModal";
import type { ProjectProjection, ProjectSummary } from "../../lib/types";
import { supportedLocales, useI18n, type Locale } from "../../i18n";
import {
  supportedFontSizes,
  useDisplayPreferences,
  type FontSize,
} from "../../preferences/DisplayPreferences";
import { AppUpdater } from "../updater/AppUpdater";
import { ProjectSwitcherModal } from "./ProjectSwitcherModal";

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
  onRenameFile,
}: Props) {
  const { locale, setLocale, t } = useI18n();
  const { fontSize, setFontSize } = useDisplayPreferences();
  const [switchingProject, setSwitchingProject] = useState(false);
  const [renameTarget, setRenameTarget] = useState<
    { kind: "file"; projectId: string; path: string; name: string }
    | null
  >(null);
  const selectedProject = projects.find((project) => project.id === selectedProjectId) ?? null;
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          <img src="/brand/env-manager-logo-v1.png" alt="" />
        </span>
        <span>Env Manager</span>
      </div>

      <section className="current-project-panel" aria-label={t("projectSwitcher.currentProject")}>
        <div className="current-project-identity">
          <span className="project-glyph" aria-hidden="true">
            {selectedProject?.name.slice(0, 1).toUpperCase() ?? "—"}
          </span>
          <span className="current-project-copy">
            <strong>{selectedProject?.name ?? t("projectSwitcher.noneSelected")}</strong>
          </span>
        </div>
        <button className="project-change-button" type="button" onClick={() => setSwitchingProject(true)}>
          <span className="project-change-label">
            {selectedProject ? t("projectSwitcher.change") : t("projectSwitcher.add")}
          </span>
          <span className="project-change-icon" aria-hidden="true">↕</span>
        </button>
      </section>

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
        <div className="sidebar-preferences">
          <label className="sidebar-select-control language-control">
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
          <label className="sidebar-select-control font-size-control">
            <span>{t("fontSize.label")}</span>
            <select
              value={fontSize}
              aria-label={t("fontSize.label")}
              onChange={(event) => setFontSize(event.target.value as FontSize)}
            >
              {supportedFontSizes.map((size) => (
                <option value={size} key={size}>
                  {t(fontSizeLabel(size))}
                </option>
              ))}
            </select>
          </label>
        </div>
        <AppUpdater />
      </div>
      {renameTarget && (
        <RenameModal
          title={t("sidebar.fileNamePrompt")}
          currentName={renameTarget.name}
          onClose={() => setRenameTarget(null)}
          onRename={(name) => {
            onRenameFile(renameTarget.projectId, renameTarget.path, name);
          }}
        />
      )}
      {switchingProject && (
        <ProjectSwitcherModal
          projects={projects}
          selectedProjectId={selectedProjectId}
          onClose={() => setSwitchingProject(false)}
          onRegister={onRegister}
          onSelectProject={onSelectProject}
        />
      )}
    </aside>
  );
}

function fontSizeLabel(fontSize: FontSize) {
  const labels = {
    small: "fontSize.small",
    medium: "fontSize.medium",
    large: "fontSize.large",
    "extra-large": "fontSize.extraLarge",
  } as const;
  return labels[fontSize];
}
