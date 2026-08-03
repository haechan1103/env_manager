import { useCallback, useEffect, useState } from "react";

import * as api from "../../lib/api";
import type {
  AgentIntegrationId,
  AgentIntegrationStatus,
} from "../../lib/types";

interface Props {
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

const marks: Record<AgentIntegrationId, string> = {
  codex: "C",
  "claude-code": "A",
  "github-copilot": "G",
};

export function AgentIntegrations({ onError, onNotice }: Props) {
  const [items, setItems] = useState<AgentIntegrationStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<AgentIntegrationId | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.listAgentIntegrations());
    } catch (error) {
      onError(error instanceof Error ? error.message : "AI 도구 연결 상태를 확인하지 못했습니다.");
    } finally {
      setLoading(false);
    }
  }, [onError]);

  useEffect(() => {
    void load();
  }, [load]);

  const install = async (item: AgentIntegrationStatus) => {
    setInstalling(item.id);
    try {
      const result = await api.installAgentIntegration(item.id);
      setItems((current) => current.map((entry) => (entry.id === result.id ? result : entry)));
      onNotice(`${item.name}에 Env Manager ${result.currentVersion} 연동을 설치했습니다.`);
    } catch (error) {
      onError(error instanceof Error ? error.message : `${item.name} 연동에 실패했습니다.`);
    } finally {
      setInstalling(null);
    }
  };

  return (
    <section className="integration-page">
      <div className="integration-intro">
        <div>
          <p className="eyebrow">ONE LOCAL BUNDLE</p>
          <h2>쓰는 도구에 같은 규칙을 연결하세요</h2>
          <p>
            하나의 Skill과 로컬 broker를 Codex, Claude Code, GitHub Copilot에서
            함께 사용합니다. 환경변수 원문은 일반 대화 대신 Env Manager 도구를 통해 다룹니다.
          </p>
        </div>
        <button className="quiet-button" onClick={() => void load()} disabled={loading}>
          {loading ? "확인 중…" : "상태 새로고침"}
        </button>
      </div>

      <div className="integration-grid" aria-live="polite">
        {items.map((item) => {
          const busy = installing === item.id;
          const actionLabel = item.updateAvailable
            ? "업데이트"
            : item.installed
              ? "설치됨"
              : "연결 설치";
          return (
            <article className={`integration-card ${item.installed ? "connected" : ""}`} key={item.id}>
              <header>
                <span className={`integration-mark ${item.id}`}>{marks[item.id]}</span>
                <div>
                  <h3>{item.name}</h3>
                  <span className={`integration-state ${item.installed ? "installed" : item.detected ? "detected" : "missing"}`}>
                    {item.installed ? "연결됨" : item.detected ? "도구 감지됨" : "도구 미설치"}
                  </span>
                </div>
              </header>

              <p className="integration-detail">{item.detail}</p>

              <dl className="integration-meta">
                <div>
                  <dt>연동 버전</dt>
                  <dd>{item.installedVersion ?? "—"}{item.updateAvailable ? ` → ${item.currentVersion}` : ""}</dd>
                </div>
                <div>
                  <dt>보호 방식</dt>
                  <dd>{protectionLabel(item.protection)}</dd>
                </div>
              </dl>

              <button
                className={item.installed && !item.updateAvailable ? "quiet-button integration-action" : "primary-button integration-action"}
                disabled={!item.canInstall || busy || (item.installed && !item.updateAvailable)}
                onClick={() => void install(item)}
              >
                {busy ? "설치 중…" : actionLabel}
              </button>
            </article>
          );
        })}
        {loading && items.length === 0 && <div className="integration-loading">연결 가능한 도구를 확인하고 있어요.</div>}
      </div>

      <div className="integration-footnote">
        <span>i</span>
        <p>
          Claude Code와 Copilot의 Guard는 직접 <code>.env</code> 접근을 막는 방어 계층입니다.
          운영체제 수준의 완전 격리는 아니므로 실제 값은 계속 보호 상태로 두는 것을 권장합니다.
        </p>
      </div>
    </section>
  );
}

function protectionLabel(protection: AgentIntegrationStatus["protection"]) {
  if (protection === "broker") return "로컬 broker";
  if (protection === "guarded") return "broker + 접근 Guard";
  return "연결 전";
}
