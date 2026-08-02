import { useEffect, useState } from "react";

import { FileEditor } from "../features/file-editor/FileEditor";
import { Overview } from "../features/overview/Overview";
import { ProjectSidebar } from "../features/projects/ProjectSidebar";
import { useEnvManager } from "../hooks/useEnvManager";

type View = { kind: "overview" } | { kind: "file"; path: string };

export function App() {
  const manager = useEnvManager();
  const [view, setView] = useState<View>({ kind: "overview" });

  useEffect(() => {
    setView({ kind: "overview" });
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
      />

      <main className="main-panel">
        {manager.loading ? (
          <div className="center-state" aria-live="polite">
            <span className="spinner" />
            <p>등록된 프로젝트를 확인하고 있어요.</p>
          </div>
        ) : !manager.selectedProject || !manager.projection ? (
          <section className="empty-project-page">
            <header className="empty-project-header">
              <div>
                <p className="eyebrow">PROJECTS</p>
                <h1>프로젝트</h1>
                <p>환경변수를 관리할 로컬 프로젝트를 등록합니다.</p>
              </div>
              <button className="primary-button" onClick={() => void manager.register()}>
                + 프로젝트 등록
              </button>
            </header>

            <div className="onboarding-layout">
              <section className="register-project-card">
                <div className="folder-mark" aria-hidden="true">
                  <span />
                </div>
                <div className="register-project-copy">
                  <p className="eyebrow">NO PROJECTS YET</p>
                  <h2>관리할 프로젝트 폴더를 선택하세요</h2>
                  <p>
                    폴더 안의 env 파일을 찾아 목록으로 보여줍니다.
                    등록하는 것만으로 파일 내용이 바뀌지는 않습니다.
                  </p>
                </div>
                <button className="primary-button large" onClick={() => void manager.register()}>
                  폴더 선택…
                </button>
              </section>

              <aside className="registration-details">
                <header>
                  <span>등록 후 동작</span>
                  <small>LOCAL ONLY</small>
                </header>
                <dl>
                  <div>
                    <dt>01</dt>
                    <dd>
                      <strong>env 파일 자동 발견</strong>
                      <span>하위 앱 폴더까지 한 번만 훑어봅니다.</span>
                    </dd>
                  </div>
                  <div>
                    <dt>02</dt>
                    <dd>
                      <strong>값은 기본적으로 가림</strong>
                      <span>이름과 입력 여부부터 확인합니다.</span>
                    </dd>
                  </div>
                  <div>
                    <dt>03</dt>
                    <dd>
                      <strong>저장할 때만 원본 반영</strong>
                      <span>별도 저장소로 값을 옮기지 않습니다.</span>
                    </dd>
                  </div>
                </dl>
              </aside>
            </div>

            <section className="file-support-strip">
              <div>
                <span className="strip-label">자동 발견</span>
                <code>.env</code>
                <code>.env.local</code>
                <code>.env.development</code>
                <code>apps/*/.env</code>
              </div>
              <p><code>.env.example</code> 계열은 V1에서 제외</p>
            </section>

            <p className="local-footnote">
              <span className="status-dot" />
              네트워크 연결 없이 이 컴퓨터에서만 동작합니다.
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
                <button className="quiet-button" onClick={() => void refresh()}>
                  새로고침
                </button>
                <button
                  className="danger-quiet-button"
                  onClick={() => {
                    if (
                      window.confirm(
                        "앱에서 등록만 제거합니다. 프로젝트 파일은 삭제하지 않습니다.",
                      )
                    ) {
                      void manager.remove(manager.selectedProject!.id);
                    }
                  }}
                >
                  등록 제거
                </button>
              </div>
            </header>

            <div className="content-scroll">
              {view.kind === "overview" && (
                <Overview
                  projection={manager.projection}
                  onOpenFile={(path) => setView({ kind: "file", path })}
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
                />
              )}
            </div>
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
    </div>
  );
}
