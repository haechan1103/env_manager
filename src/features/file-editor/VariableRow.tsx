import { useEffect, useLayoutEffect, useRef, useState } from "react";

import * as api from "../../lib/api";
import type { CodexAccess, OccurrenceProjection } from "../../lib/types";

interface Props {
  projectId: string;
  file: string;
  variable: OccurrenceProjection;
  currentGroup: string;
  groups: string[];
  sameKeyFiles: string[];
  onMutate: (operation: () => Promise<unknown>, success: string) => Promise<void>;
  onLink: () => void;
}

export function VariableRow({
  projectId,
  file,
  variable,
  currentGroup,
  groups,
  sameKeyFiles,
  onMutate,
  onLink,
}: Props) {
  const [draft, setDraft] = useState("");
  const [dirty, setDirty] = useState(false);
  const [revealed, setRevealed] = useState<string | null>(null);
  const [revealActivity, setRevealActivity] = useState(0);
  const [keyCopied, setKeyCopied] = useState(false);
  const [editingDescription, setEditingDescription] = useState(false);
  const [description, setDescription] = useState(variable.description.join("\n"));
  const revealedValueRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setDescription(variable.description.join("\n"));
  }, [variable.description]);

  useEffect(() => {
    if (revealed === null) return;
    const timeout = window.setTimeout(() => setRevealed(null), 30000);
    return () => window.clearTimeout(timeout);
  }, [revealed, revealActivity]);

  useEffect(() => {
    if (!keyCopied) return;
    const timeout = window.setTimeout(() => setKeyCopied(false), 1600);
    return () => window.clearTimeout(timeout);
  }, [keyCopied]);

  useLayoutEffect(() => {
    const field = revealedValueRef.current;
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${Math.min(field.scrollHeight, 240)}px`;
  }, [revealed, draft, dirty]);

  const value = dirty ? draft : revealed ?? "";
  const placeholder =
    variable.valueState === "present" ? "값 있음  ••••••••••••" : "값을 입력하세요";
  const hasIndependentPeers = variable.linkId === null && sameKeyFiles.length > 1;
  const unlinkedPeerFiles =
    variable.linkId === null
      ? []
      : sameKeyFiles.filter((path) => !variable.linkedFiles.includes(path));

  const keepRevealActive = () => {
    if (revealed !== null) setRevealActivity((activity) => activity + 1);
  };

  const changeAccess = async (access: CodexAccess) => {
    const downgrade = access === "read-write" && variable.codexAccess !== "read-write";
    if (
      downgrade &&
      !window.confirm(
        `${variable.key}의 실제 값을 연결된 AI 도구가 명시적 Env Manager 도구로 읽고 수정할 수 있게 합니다. 계속할까요?`,
      )
    ) {
      return;
    }
    await onMutate(
      () => api.setCodexAccess(projectId, variable.key, access, downgrade),
      `${variable.key}의 AI 접근 정책을 변경했습니다.`,
    );
  };

  return (
    <article className={variable.duplicate ? "variable-row has-error" : "variable-row"}>
      <div className="variable-main">
        <div className="variable-meta">
          <div className="key-line">
            <strong>{variable.key}</strong>
            <button
              className={keyCopied ? "key-copy-button copied" : "key-copy-button"}
              aria-label={`${variable.key} 환경변수명 복사`}
              title={keyCopied ? "복사됨" : "환경변수명 복사"}
              onClick={() => {
                void api
                  .copyKey(projectId, variable.key)
                  .then(() => setKeyCopied(true));
              }}
            >
              {keyCopied ? "✓" : "⧉"}
            </button>
            {variable.linkedCount > 1 && (
              <span className="badge linked">{variable.linkedCount}개 파일 연결됨</span>
            )}
            {hasIndependentPeers && (
              <span className="badge available">같은 변수 {sameKeyFiles.length}곳</span>
            )}
            {variable.duplicate && <span className="badge error">중복 키</span>}
          </div>
          {variable.description.length > 0 ? (
            <button className="description-button" onClick={() => setEditingDescription((open) => !open)}>
              {variable.description.join(" ")}
            </button>
          ) : (
            <button className="description-button muted" onClick={() => setEditingDescription(true)}>설명 추가</button>
          )}
        </div>

        <div
          className="value-editor"
          onFocus={keepRevealActive}
          onKeyDown={keepRevealActive}
          onPointerDown={keepRevealActive}
          onWheel={keepRevealActive}
        >
          {revealed !== null ? (
            <textarea
              ref={revealedValueRef}
              className="revealed-value-field"
              value={value}
              aria-label={`${variable.key} 값`}
              rows={1}
              onChange={(event) => {
                setDraft(event.target.value);
                setDirty(true);
                keepRevealActive();
              }}
            />
          ) : (
            <input
              type="password"
              value={value}
              placeholder={placeholder}
              aria-label={`${variable.key} 값`}
              onChange={(event) => {
                setDraft(event.target.value);
                setDirty(true);
              }}
            />
          )}
          <button
            className="icon-button"
            title={revealed === null ? "값 보기 · 30초 미활동 시 숨김" : "값 숨기기"}
            onClick={() => {
              if (revealed !== null) {
                setRevealed(null);
                return;
              }
              if (dirty) {
                setRevealed(draft);
                setRevealActivity((activity) => activity + 1);
              } else {
                void api
                  .readValue(projectId, file, variable.key)
                  .then((nextValue) => {
                    setRevealed(nextValue);
                    setRevealActivity((activity) => activity + 1);
                  })
                  .catch(() => setRevealed(null));
              }
            }}
          >
            {revealed === null ? "◉" : "○"}
          </button>
          <button
            className="icon-button"
            title="값 복사"
            onClick={() => void api.copyValue(projectId, file, variable.key)}
          >
            ⧉
          </button>
        </div>

        <div className="variable-actions">
          <select
            className={`access-select ${variable.codexAccess}`}
            value={variable.codexAccess}
            aria-label={`${variable.key} AI 접근`}
            onChange={(event) => void changeAccess(event.target.value as CodexAccess)}
          >
            <option value="protected">보호됨</option>
            <option value="unclassified">미분류</option>
            <option value="read-write">AI 허용</option>
          </select>
          {dirty ? (
            <button
              className="primary-button compact"
              disabled={variable.duplicate}
              onClick={() =>
                void onMutate(
                  () => api.saveValue(projectId, { file, key: variable.key, newValue: draft }),
                  variable.linkedCount > 1
                    ? `${variable.linkedCount}개 파일에 저장했습니다.`
                    : `${variable.key} 값을 저장했습니다.`,
                ).then(() => {
                  setDirty(false);
                  setDraft("");
                })
              }
            >
              {variable.linkedCount > 1 ? `${variable.linkedCount}개 파일에 저장` : "저장"}
            </button>
          ) : null}
          <button
            className="quiet-button compact"
            title="다른 그룹으로 이동"
            onClick={() => {
              const choices = groups.filter((group) => group !== currentGroup);
              if (choices.length === 0) return;
              const target = window
                .prompt(`이동할 그룹 이름\n${choices.join(" · ")}`, choices[0])
                ?.trim();
              if (!target || !choices.includes(target)) return;
              void onMutate(
                () =>
                  api.moveVariable(projectId, {
                    file,
                    key: variable.key,
                    targetGroup: target,
                  }),
                `${variable.key}를 ${target} 그룹으로 이동했습니다.`,
              );
            }}
          >
            이동
          </button>
          <button
            className="danger-quiet-button compact"
            title="변수와 바로 위 설명 삭제"
            onClick={() => {
              if (
                window.confirm(
                  `${file}에서 ${variable.key}와 바로 위 설명을 삭제합니다. 이 작업은 되돌릴 수 없습니다.`,
                )
              ) {
                void onMutate(
                  () => api.deleteVariable(projectId, { file, key: variable.key }),
                  `${variable.key}를 삭제했습니다.`,
                );
              }
            }}
          >
            삭제
          </button>
        </div>
      </div>

      {variable.linkId && variable.linkedFiles.length > 1 && (
        <div className="relationship-panel linked-relationship">
          <span className="relationship-icon" aria-hidden="true">↔</span>
          <div className="relationship-copy">
            <strong>{variable.linkedFiles.length}개 파일에서 함께 관리</strong>
            <span>어느 파일에서 값을 바꿔도 아래 파일에 모두 저장됩니다.</span>
            <div className="relationship-paths">
              {variable.linkedFiles.map((path) => (
                <code className={path === file ? "current" : ""} key={path}>
                  {path}
                  {path === file && <small>현재</small>}
                </code>
              ))}
            </div>
            {unlinkedPeerFiles.length > 0 && (
              <span className="unlinked-peer-note">
                별도 관리 중: {unlinkedPeerFiles.join(" · ")}
              </span>
            )}
          </div>
          <button
            className="quiet-button compact relationship-action"
            onClick={() => {
              if (window.confirm("현재 파일만 연결에서 분리하고 값은 그대로 유지합니다.")) {
                void onMutate(
                  () => api.detachLink(projectId, variable.linkId!, file),
                  `${file}을 연결에서 분리했습니다.`,
                );
              }
            }}
          >
            이 파일 연결 해제
          </button>
        </div>
      )}

      {hasIndependentPeers && (
        <div className="relationship-panel available-relationship">
          <span className="relationship-icon" aria-hidden="true">＋</span>
          <div className="relationship-copy">
            <strong>같은 변수가 {sameKeyFiles.length}개 파일에 있습니다</strong>
            <span>현재는 각각 따로 관리됩니다. 연결하면 한 번의 입력으로 함께 저장할 수 있어요.</span>
            <div className="relationship-paths">
              {sameKeyFiles.map((path) => (
                <code className={path === file ? "current" : ""} key={path}>
                  {path}
                  {path === file && <small>현재</small>}
                </code>
              ))}
            </div>
          </div>
          <button className="secondary-button compact relationship-action" onClick={onLink}>
            함께 관리
          </button>
        </div>
      )}

      {editingDescription && (
        <div className="description-editor">
          <textarea value={description} onChange={(event) => setDescription(event.target.value)} />
          <div>
            <button className="quiet-button compact" onClick={() => setEditingDescription(false)}>취소</button>
            <button
              className="secondary-button compact"
              onClick={() =>
                void onMutate(
                  () =>
                    api.saveDescription(projectId, {
                      file,
                      key: variable.key,
                      lines: description.trim() ? description.split("\n") : [],
                    }),
                  `${variable.key} 설명을 저장했습니다.`,
                ).then(() => setEditingDescription(false))
              }
            >
              설명 저장
            </button>
          </div>
        </div>
      )}
    </article>
  );
}
