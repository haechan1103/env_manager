import { useMemo, useState } from "react";

import * as api from "../../lib/api";
import type {
  EffectiveContext,
  EffectiveProjection,
  FrameworkKind,
  ProjectProjection,
} from "../../lib/types";

export function EffectiveValues({
  projectId,
  projection,
  onError,
}: {
  projectId: string;
  projection: ProjectProjection;
  onError: (message: string) => void;
}) {
  const keys = useMemo(
    () =>
      [...new Set(projection.files.flatMap((file) => file.groups.flatMap((group) => group.variables.map((variable) => variable.key))))].sort(),
    [projection],
  );
  const [key, setKey] = useState(keys[0] ?? "");
  const [framework, setFramework] = useState<FrameworkKind>("next-js");
  const [mode, setMode] = useState("development");
  const [directory, setDirectory] = useState(".");
  const [result, setResult] = useState<EffectiveProjection | null>(null);

  const inspect = async () => {
    const context: EffectiveContext = {
      framework,
      mode,
      workingDirectory: directory.trim() || ".",
      processKeys: [],
      customPrecedence: framework === "custom" ? projection.files.map((file) => file.path) : [],
    };
    try {
      setResult(await api.getEffectiveValue(projectId, key, context));
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : "적용값을 계산하지 못했습니다.");
    }
  };

  return (
    <section className="page-stack">
      <div className="section-heading">
        <div>
          <p className="eyebrow">EFFECTIVE VALUE</p>
          <h2>실제로 어떤 파일이 이길까요?</h2>
          <p className="section-copy">런타임을 바꾸지 않고, 확인한 프레임워크 규칙으로 적용 순서를 설명합니다.</p>
        </div>
      </div>

      <section className="effective-config panel">
        <label>프레임워크
          <select value={framework} onChange={(event) => setFramework(event.target.value as FrameworkKind)}>
            <option value="next-js">Next.js</option>
            <option value="vite">Vite</option>
            <option value="custom">사용자 지정</option>
          </select>
        </label>
        <label>모드<input value={mode} onChange={(event) => setMode(event.target.value)} placeholder="development" /></label>
        <label>작업 디렉터리<input value={directory} onChange={(event) => setDirectory(event.target.value)} placeholder="apps/web" /></label>
        <label>변수
          <select value={key} onChange={(event) => setKey(event.target.value)}>
            {keys.map((item) => <option key={item}>{item}</option>)}
          </select>
        </label>
        <button className="primary-button" onClick={() => void inspect()} disabled={!key}>적용 순서 확인</button>
      </section>

      {result ? (
        <section className="effective-result">
          <div className="winner-card">
            <span className="eyebrow">WINNER</span>
            <strong>{result.winner ?? "확인할 수 없음"}</strong>
            <p>{result.reason}</p>
          </div>
          <div className="precedence-flow">
            <h3>{result.key} 우선순위</h3>
            {result.winner && <div className="precedence-item winner"><span>1</span><strong>{result.winner}</strong><small>실제 적용 예상</small></div>}
            {result.shadowed.map((file, index) => (
              <div className="precedence-item" key={file}><span>{index + 2}</span><strong>{file}</strong><small>상위 occurrence에 의해 가려짐</small></div>
            ))}
          </div>
        </section>
      ) : (
        <div className="empty-effective">
          <span>↳</span>
          <h3>런타임 문맥을 선택하세요.</h3>
          <p>같은 키가 여러 파일에 있어도 연결과 덮어쓰기는 서로 다른 개념입니다.</p>
        </div>
      )}
    </section>
  );
}
