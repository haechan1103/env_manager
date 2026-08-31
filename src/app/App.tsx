import { useEffect, useState } from "react";

import { RenameModal } from "../components/RenameModal";
import { ActionPackModal } from "../features/action-pack/ActionPackModal";
import { FileEditor } from "../features/file-editor/FileEditor";
import { ExportEnvModal } from "../features/export/ExportEnvModal";
import { ImportEnvModal } from "../features/export/ImportEnvModal";
import { ProviderPushModal } from "../features/provider-push/ProviderPushModal";
import { TeamChannelModal } from "../features/team-channel/TeamChannelModal";
import { AgentActivity } from "../features/activity/AgentActivity";
import { AgentIntegrations } from "../features/integrations/AgentIntegrations";
import { Overview } from "../features/overview/Overview";
import { ProjectSidebar } from "../features/projects/ProjectSidebar";
import { ClassificationReview } from "../features/review/ClassificationReview";
import { useEnvManager } from "../hooks/useEnvManager";
import { useI18n } from "../i18n";

type View =
  | { kind: "overview" }
  | { kind: "file"; path: string }
  | { kind: "integrations" }
  | { kind: "activity" }
  | { kind: "review" };

export function App() {
  const { t } = useI18n();
  const manager = useEnvManager();
  const [view, setView] = useState<View>({ kind: "overview" });
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [pushing, setPushing] = useState(false);
  const [runningAction, setRunningAction] = useState(false);
  const [sharing, setSharing] = useState(false);
  const [channelPublishing, setChannelPublishing] = useState<string | null>(null);
  const [channelImporting, setChannelImporting] = useState<{ channelId: string; packageId: string } | null>(null);
  const [renamingProject, setRenamingProject] = useState(false);

  useEffect(() => {
    setView({ kind: "overview" });
    setRenamingProject(false);
    setSharing(false);
    setRunningAction(false);
    setChannelPublishing(null);
    setChannelImporting(null);
  }, [manager.selectedProjectId]);

  useEffect(() => {
    if (!manager.error && !manager.notice) return;
    const timeout = window.setTimeout(() => {
      manager.clearError();
      manager.clearNotice();
    }, 5000);
    return () => window.clearTimeout(timeout);
  }, [manager.error, manager.notice, manager.clearError, manager.clearNotice]);

  const refresh = async () => {
    if (!manager.selectedProjectId) return;
    await manager.refreshProject(manager.selectedProjectId);
  };

  return (
    <div className="app-shell">
      <ProjectSidebar
        projects={manager.projects}
        selectedProjectId={manager.selectedProjectId}
        projection={manager.projection}
        view={view}
        onSelectProject={manager.selectProject}
        onSelectView={setView}
        onRegister={() => void manager.register()}
        onRenameFile={(projectId, path, name) => void manager.renameEnvFile(projectId, path, name)}
      />

      <main className="main-panel">
        {view.kind === "integrations" ? (
          <>
            <header className="project-header integration-header">
              <div>
                <h1>{t("app.integrationsTitle")}</h1>
              </div>
            </header>
            <div className="content-scroll">
              <AgentIntegrations
                onError={manager.showError}
                onNotice={manager.showNotice}
              />
            </div>
          </>
        ) : manager.loading ? (
          <div className="center-state" aria-live="polite">
            <span className="spinner" />
            <p>{t("app.loadingProjects")}</p>
          </div>
        ) : !manager.selectedProject || !manager.projection ? (
          <section className="empty-project-page">
            <header className="empty-project-header">
              <div>
                <h1>{t("app.projectsTitle")}</h1>
                <p>{t("app.projectsSubtitle")}</p>
              </div>
              <button className="primary-button" onClick={() => void manager.register()}>
                {t("app.registerProject")}
              </button>
            </header>

            <div className="onboarding-layout">
              <section className="register-project-card">
                <div className="folder-mark" aria-hidden="true">
                  <span />
                </div>
                <div className="register-project-copy">
                  <p className="eyebrow">{t("app.noProjects")}</p>
                  <h2>{t("app.chooseFolderTitle")}</h2>
                  <p>{t("app.chooseFolderBody")}</p>
                </div>
                <button className="primary-button large" onClick={() => void manager.register()}>
                  {t("app.chooseFolder")}
                </button>
              </section>

              <aside className="registration-details">
                <header>
                  <span>{t("app.afterRegistration")}</span>
                  <small>LOCAL ONLY</small>
                </header>
                <dl>
                  <div>
                    <dt>01</dt>
                    <dd>
                      <strong>{t("app.discoveryTitle")}</strong>
                      <span>{t("app.discoveryBody")}</span>
                    </dd>
                  </div>
                  <div>
                    <dt>02</dt>
                    <dd>
                      <strong>{t("app.maskedTitle")}</strong>
                      <span>{t("app.maskedBody")}</span>
                    </dd>
                  </div>
                  <div>
                    <dt>03</dt>
                    <dd>
                      <strong>{t("app.writeTitle")}</strong>
                      <span>{t("app.writeBody")}</span>
                    </dd>
                  </div>
                </dl>
              </aside>
            </div>

            <section className="file-support-strip">
              <div>
                <span className="strip-label">{t("app.autoDiscovery")}</span>
                <code>.env</code>
                <code>.env.local</code>
                <code>.env.development</code>
                <code>.dev.vars</code>
                <code>apps/*/.env</code>
              </div>
              <p><code>.env.example</code> {t("app.examplesExcluded")}</p>
            </section>

            <p className="local-footnote">
              <span className="status-dot" />
              {t("app.localFootnote")}
            </p>
          </section>
        ) : (
          <>
            <header className="project-header">
              <div>
                <p className="eyebrow">{manager.selectedProject.displayPath}</p>
                <h1>{manager.selectedProject.name}</h1>
              </div>
              <div className="header-actions">
                <button className="quiet-button" onClick={() => {
                  setRenamingProject(true);
                }}>{t("common.rename")}</button>
                <button className="quiet-button" onClick={() => setExporting(true)}>{t("export.action")}</button>
                <button className="quiet-button" onClick={() => setImporting(true)}>{t("import.headerAction")}</button>
                <button className="quiet-button" onClick={() => setSharing(true)}>{t("teamChannel.headerAction")}</button>
                <button className="quiet-button" onClick={() => setPushing(true)}>{t("push.headerAction")}</button>
                <button className="quiet-button" onClick={() => setRunningAction(true)}>{t("action.headerAction")}</button>
                <button className="quiet-button" onClick={() => void refresh()}>
                  {t("common.refresh")}
                </button>
                <button
                  className="danger-quiet-button"
                  onClick={() => {
                    if (
                      window.confirm(
                        t("app.removeConfirm"),
                      )
                    ) {
                      void manager.remove(manager.selectedProject!.id);
                    }
                  }}
                >
                  {t("app.removeRegistration")}
                </button>
              </div>
            </header>

            <div className="content-scroll">
              {view.kind === "overview" && (
                <Overview
                  projection={manager.projection}
                  onOpenFile={(path) => setView({ kind: "file", path })}
                  onApplyGitignoreGuard={manager.applyGitignoreGuard}
                  onOpenReview={() => setView({ kind: "review" })}
                />
              )}
              {view.kind === "file" && (
                <FileEditor
                  projectId={manager.selectedProject.id}
                  projection={manager.projection}
                  filePath={view.path}
                  onRefresh={refresh}
                  onError={manager.showError}
                  onNotice={manager.showNotice}
                  onRenameFile={(path, name) => void manager.renameEnvFile(manager.selectedProject!.id, path, name)}
                />
              )}
              {view.kind === "review" && (
                <ClassificationReview
                  projectId={manager.selectedProject.id}
                  projection={manager.projection}
                  onRefresh={refresh}
                  onOpenFile={(path) => setView({ kind: "file", path })}
                  onError={manager.showError}
                  onNotice={manager.showNotice}
                />
              )}
              {view.kind === "activity" && (
                <AgentActivity projectId={manager.selectedProject.id} onError={manager.showError} />
              )}
            </div>
            {renamingProject && (
              <RenameModal
                title={t("sidebar.projectNamePrompt")}
                currentName={manager.selectedProject.name}
                onClose={() => setRenamingProject(false)}
                onRename={(name) => void manager.renameProject(manager.selectedProject!.id, name)}
              />
            )}
          </>
        )}
      </main>

      {(manager.error || manager.notice) && (
        <div
          className={`toast ${manager.error ? "toast-error" : "toast-success"}`}
          role="status"
        >
          {manager.error ?? manager.notice}
        </div>
      )}
      {exporting && manager.selectedProject && (
        <ExportEnvModal projectId={manager.selectedProject.id} projection={manager.projection!} onClose={() => setExporting(false)} onError={manager.showError} onNotice={manager.showNotice} />
      )}
      {importing && manager.selectedProject && manager.projection && (
        <ImportEnvModal
          projectId={manager.selectedProject.id}
          projection={manager.projection}
          onApplied={() => manager.refreshProject(manager.selectedProject!.id).then(() => undefined)}
          onClose={() => setImporting(false)}
          onError={manager.showError}
          onNotice={manager.showNotice}
        />
      )}
      {pushing && manager.selectedProject && manager.projection && (
        <ProviderPushModal
          projectId={manager.selectedProject.id}
          projection={manager.projection}
          onClose={() => setPushing(false)}
          onError={manager.showError}
          onNotice={manager.showNotice}
        />
      )}
      {runningAction && manager.selectedProject && manager.projection && (
        <ActionPackModal
          projectId={manager.selectedProject.id}
          projection={manager.projection}
          onClose={() => setRunningAction(false)}
          onError={manager.showError}
          onNotice={manager.showNotice}
        />
      )}
      {sharing && manager.selectedProject && (
        <TeamChannelModal
          projectId={manager.selectedProject.id}
          onClose={() => setSharing(false)}
          onError={manager.showError}
          onNotice={manager.showNotice}
          onPublish={(channelId) => {
            setSharing(false);
            setChannelPublishing(channelId);
          }}
          onImport={(channelId, packageId) => {
            setSharing(false);
            setChannelImporting({ channelId, packageId });
          }}
        />
      )}
      {channelPublishing && manager.selectedProject && manager.projection && (
        <ExportEnvModal
          projectId={manager.selectedProject.id}
          projection={manager.projection}
          channelId={channelPublishing}
          onClose={() => setChannelPublishing(null)}
          onError={manager.showError}
          onNotice={manager.showNotice}
        />
      )}
      {channelImporting && manager.selectedProject && manager.projection && (
        <ImportEnvModal
          projectId={manager.selectedProject.id}
          projection={manager.projection}
          channelSource={channelImporting}
          onApplied={() => manager.refreshProject(manager.selectedProject!.id).then(() => undefined)}
          onClose={() => setChannelImporting(null)}
          onError={manager.showError}
          onNotice={manager.showNotice}
        />
      )}
    </div>
  );
}
