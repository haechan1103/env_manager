import type { ProjectProjection } from "../../lib/types";

interface Props {
  projection: ProjectProjection;
  onOpenFile: (path: string) => void;
  onOpenEffective: () => void;
}

export function Overview({ projection, onOpenFile, onOpenEffective }: Props) {
  const variables = projection.files.flatMap((file) =>
    file.groups.flatMap((group) => group.variables.map((variable) => ({ ...variable, file: file.path }))),
  );
  const empty = variables.filter((variable) => variable.valueState === "empty");
  const linked = variables.filter((variable) => variable.linkedCount > 0).length;
  const protectedCount = variables.filter((variable) => variable.codexAccess === "protected").length;

  return (
    <section className="page-stack">
      <div className="section-heading">
        <div>
          <p className="eyebrow">PROJECT HEALTH</p>
          <h2>지금 확인할 항목</h2>
        </div>
        <button className="secondary-button" onClick={onOpenEffective}>실제 적용값 확인</button>
      </div>

      <div className="stats-grid">
        <Stat label="관리 파일" value={projection.files.length} detail="등록 프로젝트 내부" />
        <Stat label="환경변수" value={variables.length} detail={`${protectedCount}개 보호됨`} />
        <Stat label="연결 occurrence" value={linked} detail="명시적 peer link" />
        <Stat
          label="조치 필요"
          value={empty.length + projection.unclassifiedCount + projection.issueCount}
          detail="입력 · 분류 · 파싱"
          tone={empty.length + projection.unclassifiedCount + projection.issueCount > 0 ? "warn" : "good"}
        />
      </div>

      <div className="overview-grid">
        <section className="panel">
          <div className="panel-title">
            <h3>Action inbox</h3>
            <span>{empty.length + projection.unclassifiedCount + projection.issueCount}</span>
          </div>
          {empty.length === 0 && projection.unclassifiedCount === 0 && projection.issueCount === 0 ? (
            <div className="empty-inline">
              <span>✓</span>
              <p>지금 바로 처리할 문제가 없습니다.</p>
            </div>
          ) : (
            <div className="issue-list">
              {empty.map((variable) => (
                <button key={`${variable.file}:${variable.key}`} onClick={() => onOpenFile(variable.file)}>
                  <span className="issue-icon amber">○</span>
                  <span><strong>{variable.key}</strong><small>{variable.file} · 값 입력 필요</small></span>
                  <span>→</span>
                </button>
              ))}
              {projection.unclassifiedCount > 0 && (
                <div className="issue-row">
                  <span className="issue-icon violet">?</span>
                  <span><strong>Codex 접근 미분류</strong><small>{projection.unclassifiedCount}개 변수 검토 필요</small></span>
                </div>
              )}
              {projection.issueCount > 0 && (
                <div className="issue-row">
                  <span className="issue-icon red">!</span>
                  <span><strong>파일 파싱 경고</strong><small>{projection.issueCount}개 보존된 경고</small></span>
                </div>
              )}
            </div>
          )}
        </section>

        <section className="panel">
          <div className="panel-title"><h3>관리 파일</h3><span>{projection.files.length}</span></div>
          <div className="managed-files">
            {projection.files.map((file) => {
              const count = file.groups.reduce((total, group) => total + group.variables.length, 0);
              return (
                <button key={file.path} onClick={() => onOpenFile(file.path)}>
                  <span className="file-icon">ENV</span>
                  <span><strong>{file.path}</strong><small>{count} variables · {file.groups.length} groups</small></span>
                  <span>→</span>
                </button>
              );
            })}
          </div>
        </section>
      </div>

      <div className="security-note">
        <span>⌾</span>
        <div>
          <strong>값은 프로젝트 파일에만 남습니다.</strong>
          <p>Env Manager는 별도 vault로 가져오지 않으며, 일반 화면과 Codex inspection에는 원문 값을 전달하지 않습니다.</p>
        </div>
      </div>
    </section>
  );
}

function Stat({ label, value, detail, tone }: { label: string; value: number; detail: string; tone?: "warn" | "good" }) {
  return (
    <article className={`stat-card ${tone ?? ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}
