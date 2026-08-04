import type { ProjectProjection, ProjectSummary } from "../../lib/types";

interface View {
  kind: "overview" | "file" | "integrations";
  path?: string;
}

interface Props {
  projects: ProjectSummary[];
  selectedProjectId: string | null;
  projection: ProjectProjection | null;
  view: View;
  onSelectProject: (projectId: string) => void;
  onSelectView: (
    view: { kind: "overview" } | { kind: "file"; path: string } | { kind: "integrations" },
  ) => void;
  onRegister: () => void;
}

export function ProjectSidebar({
  projects,
  selectedProjectId,
  projection,
  view,
  onSelectProject,
  onSelectView,
  onRegister,
}: Props) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">E</span>
        <span>Env Manager</span>
        <span className="version">v0.2.1</span>
      </div>

      <div className="sidebar-section-title">
        <span>PROJECTS</span>
        <button aria-label="프로젝트 등록" onClick={onRegister}>+</button>
      </div>

      <nav className="project-list" aria-label="등록 프로젝트">
        {projects.map((project) => (
          <button
            key={project.id}
            className={project.id === selectedProjectId ? "project-item selected" : "project-item"}
            onClick={() => onSelectProject(project.id)}
          >
            <span className="project-glyph">{project.name.slice(0, 1).toUpperCase()}</span>
            <span className="project-name">{project.name}</span>
          </button>
        ))}
        {projects.length === 0 && (
          <div className="sidebar-empty">
            <span>—</span>
            등록된 프로젝트 없음
          </div>
        )}
      </nav>

      <nav className="agent-navigation" aria-label="AI 도구">
        <button
          className={view.kind === "integrations" ? "nav-item active" : "nav-item"}
          onClick={() => onSelectView({ kind: "integrations" })}
        >
          <span>◇</span>
          <span className="agent-nav-label">AI 도구 연결</span>
        </button>
      </nav>

      {projection && (
        <nav className="file-navigation" aria-label="프로젝트 보기">
          <button
            className={view.kind === "overview" ? "nav-item active" : "nav-item"}
            onClick={() => onSelectView({ kind: "overview" })}
          >
            <span>⌁</span> Overview
            {(projection.issueCount > 0 || projection.unclassifiedCount > 0) && (
              <b>{projection.issueCount + projection.unclassifiedCount}</b>
            )}
          </button>
          <p className="file-label">ENV FILES</p>
          {projection.files.map((file) => (
            <button
              key={file.path}
              className={view.kind === "file" && view.path === file.path ? "nav-item active" : "nav-item"}
              onClick={() => onSelectView({ kind: "file", path: file.path })}
              title={file.path}
            >
              <span className="file-dot" />
              <span className="truncate">{file.path}</span>
              {file.warnings.length > 0 && <b>!</b>}
            </button>
          ))}
        </nav>
      )}

      <div className="sidebar-footer">
        <span className="status-dot" />
        <span>Local only</span>
      </div>
    </aside>
  );
}
