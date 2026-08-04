import { useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import * as api from "../../lib/api";
import type {
  FileProjection,
  MigrationPlanProjection,
  OccurrenceProjection,
  ProjectProjection,
} from "../../lib/types";
import { VariableRow } from "./VariableRow";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  filePath: string;
  onRefresh: () => Promise<void>;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function FileEditor({
  projectId,
  projection,
  filePath,
  onRefresh,
  onError,
  onNotice,
}: Props) {
  const file = projection.files.find((item) => item.path === filePath);
  const [adding, setAdding] = useState(false);
  const [addingGroup, setAddingGroup] = useState(false);
  const [linking, setLinking] = useState<OccurrenceProjection | null>(null);
  const [migration, setMigration] = useState<MigrationPlanProjection | null>(null);

  const occurrenceFilesByKey = useMemo(() => {
    const filesByKey = new Map<string, Set<string>>();
    for (const candidate of projection.files) {
      for (const group of candidate.groups) {
        for (const variable of group.variables) {
          const files = filesByKey.get(variable.key) ?? new Set<string>();
          files.add(candidate.path);
          filesByKey.set(variable.key, files);
        }
      }
    }
    return filesByKey;
  }, [projection.files]);

  const sameKeyFiles = useMemo(() => {
    if (!linking) return [];
    const candidatePaths = new Set(occurrenceFilesByKey.get(linking.key) ?? []);
    return projection.files.filter((candidate) =>
      candidatePaths.has(candidate.path),
    );
  }, [linking, occurrenceFilesByKey, projection.files]);

  if (!file) {
    return (
      <section className="center-state">
        <p>파일이 제거되었거나 아직 새로고침되지 않았습니다.</p>
      </section>
    );
  }

  const mutate = async (operation: () => Promise<unknown>, success: string) => {
    try {
      await operation();
      await onRefresh();
      onNotice(success);
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : "변경을 적용하지 못했습니다.");
    }
  };

  return (
    <section className="page-stack">
      <div className="file-heading">
        <div>
          <p className="eyebrow">ENV FILE</p>
          <h2>{file.path}</h2>
          <p>{countVariables(file)}개 변수 · {file.groups.length}개 그룹</p>
        </div>
        <div className="header-actions">
          <button
            className="quiet-button"
            onClick={() => {
              void api
                .planMigration(projectId, file.path)
                .then(setMigration)
                .catch((cause: unknown) =>
                  onError(
                    cause instanceof Error
                      ? cause.message
                      : "정리할 그룹 표식을 찾지 못했습니다.",
                  ),
                );
            }}
          >
            기존 주석 정리
          </button>
          <button className="quiet-button" onClick={() => setAddingGroup(true)}>+ 새 그룹</button>
          <button className="primary-button" onClick={() => setAdding(true)}>+ 새 변수</button>
        </div>
      </div>

      {file.warnings.length > 0 && (
        <div className="warning-banner">
          <strong>파일 내용을 모두 보존했지만 확인이 필요한 줄이 있습니다.</strong>
          <span>{file.warnings.join(" ")}</span>
        </div>
      )}

      <div className="groups-stack">
        {file.groups.map((group, groupIndex) => (
          <section className="group-card" key={`${group.name}:${groupIndex}`}>
            <header className="group-header">
              <div>
                <span className="group-fold">⌄</span>
                <h3>{group.name}</h3>
                <span>{group.variables.length}</span>
              </div>
              {group.name !== "기타" && (
                <button
                  className="quiet-button compact"
                  onClick={() => {
                    const name = window.prompt("새 그룹 이름", group.name)?.trim();
                    if (!name || name === group.name) return;
                    void mutate(
                      () =>
                        api.renameGroup(projectId, {
                          file: file.path,
                          currentName: group.name,
                          newName: name,
                        }),
                      "그룹 이름을 변경했습니다.",
                    );
                  }}
                >
                  이름 변경
                </button>
              )}
            </header>
            <div className="variables-table">
              {group.variables.length === 0 && (
                <div className="empty-group-row">아직 변수가 없습니다. 새 변수를 이 그룹에 추가해보세요.</div>
              )}
              {group.variables.map((variable) => (
                <VariableRow
                  key={variable.key}
                  projectId={projectId}
                  file={file.path}
                  variable={variable}
                  currentGroup={group.name}
                  groups={file.groups.map((item) => item.name)}
                  sameKeyFiles={[...(occurrenceFilesByKey.get(variable.key) ?? [file.path])]}
                  onMutate={mutate}
                  onLink={() => setLinking(variable)}
                />
              ))}
            </div>
          </section>
        ))}
      </div>

      {adding && (
        <AddVariableModal
          file={file}
          onClose={() => setAdding(false)}
          onSubmit={(request) => {
            void mutate(
              () => api.addVariable(projectId, { file: file.path, ...request }),
              `${request.key} 변수를 추가했습니다.`,
            ).then(() => setAdding(false));
          }}
        />
      )}

      {addingGroup && (
        <CreateGroupModal
          file={file.path}
          onClose={() => setAddingGroup(false)}
          onSubmit={(name) => {
            void mutate(
              () => api.createGroup(projectId, { file: file.path, name }),
              `${name} 그룹을 만들었습니다.`,
            ).then(() => setAddingGroup(false));
          }}
        />
      )}

      {linking && (
        <LinkModal
          currentFile={file.path}
          variable={linking}
          candidates={sameKeyFiles}
          onClose={() => setLinking(null)}
          onSubmit={(files) => {
            void mutate(
              () =>
                api.createLink(projectId, {
                  key: linking.key,
                  files,
                  sourceFile: file.path,
                }),
              `${linking.key}를 ${files.length}개 파일에서 연결했습니다.`,
            ).then(() => setLinking(null));
          }}
        />
      )}

      {migration && (
        <MigrationModal
          plan={migration}
          onClose={() => setMigration(null)}
          onApply={() => {
            void mutate(
              () => api.applyMigration(projectId, migration.planId),
              `${migration.preview.file}의 그룹 표식을 정리했습니다.`,
            ).then(() => setMigration(null));
          }}
        />
      )}
    </section>
  );
}

function MigrationModal({
  plan,
  onClose,
  onApply,
}: {
  plan: MigrationPlanProjection;
  onClose: () => void;
  onApply: () => void;
}) {
  return (
    <Modal
      title="기존 env 주석 정리 계획"
      description={`${plan.preview.file} · ${plan.expiresInSeconds / 60}분 동안 유효`}
      onClose={onClose}
    >
      <div className="migration-summary">
        <p>{plan.preview.summary}</p>
        <div className="migration-list">
          {plan.preview.suggestions.map((suggestion) => (
            <div key={`${suggestion.currentMarker}:${suggestion.groupName}`}>
              <code>{suggestion.currentMarker}</code>
              <span>→</span>
              <code># @group {suggestion.groupName}</code>
            </div>
          ))}
        </div>
        <div className="impact-note">
          <strong>값은 계획과 화면에 포함되지 않습니다.</strong>
          <span>파일이 미리보기 이후 바뀌었다면 적용을 중단하고 새 계획을 요청합니다.</span>
        </div>
      </div>
      <div className="modal-actions">
        <button className="quiet-button" onClick={onClose}>취소</button>
        <button className="primary-button" onClick={onApply}>계획 적용</button>
      </div>
    </Modal>
  );
}

function countVariables(file: FileProjection) {
  return file.groups.reduce((total, group) => total + group.variables.length, 0);
}

function CreateGroupModal({
  file,
  onClose,
  onSubmit,
}: {
  file: string;
  onClose: () => void;
  onSubmit: (name: string) => void;
}) {
  const [name, setName] = useState("");
  return (
    <Modal
      title="새 그룹"
      description={`${file}에 # @group 표식을 추가합니다.`}
      onClose={onClose}
    >
      <form
        className="modal-form"
        onSubmit={(event) => {
          event.preventDefault();
          const trimmed = name.trim();
          if (!trimmed) return;
          onSubmit(trimmed);
        }}
      >
        <label>
          그룹 이름
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="GPT"
            autoFocus
          />
        </label>
        <div className="impact-note">
          <strong>기존 변수와 값은 바꾸지 않습니다.</strong>
          <span>그룹을 만든 뒤 변수를 추가하거나 이동할 수 있습니다.</span>
        </div>
        <div className="modal-actions">
          <button type="button" className="quiet-button" onClick={onClose}>취소</button>
          <button className="primary-button" disabled={!name.trim()}>그룹 만들기</button>
        </div>
      </form>
    </Modal>
  );
}

function AddVariableModal({
  file,
  onClose,
  onSubmit,
}: {
  file: FileProjection;
  onClose: () => void;
  onSubmit: (request: {
    key: string;
    group: string;
    description: string[];
    value: string;
  }) => void;
}) {
  const [key, setKey] = useState("");
  const [group, setGroup] = useState(file.groups[0]?.name ?? "기타");
  const [newGroup, setNewGroup] = useState("");
  const [description, setDescription] = useState("");
  const [value, setValue] = useState("");
  return (
    <Modal
      title="새 환경변수"
      description={`${file.path}에 빈 값 또는 입력한 값을 추가합니다.`}
      onClose={onClose}
    >
      <form
        className="modal-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!key.trim()) return;
          const targetGroup = group === "__new_group__" ? newGroup.trim() : group;
          if (!targetGroup) return;
          onSubmit({
            key: key.trim().toUpperCase(),
            group: targetGroup,
            description: description.trim() ? description.split("\n") : [],
            value,
          });
        }}
      >
        <label>변수 이름<input value={key} onChange={(event) => setKey(event.target.value)} placeholder="NEW_VARIABLE" autoFocus /></label>
        <label>그룹
          <select value={group} onChange={(event) => setGroup(event.target.value)}>
            {file.groups.map((item) => <option key={item.name}>{item.name}</option>)}
            {!file.groups.some((item) => item.name === "기타") && <option value="기타">기타 (그룹 없음)</option>}
            <option value="__new_group__">+ 새 그룹 만들기</option>
          </select>
        </label>
        {group === "__new_group__" && (
          <label>새 그룹 이름<input value={newGroup} onChange={(event) => setNewGroup(event.target.value)} placeholder="GPT" /></label>
        )}
        <label>설명<textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="한 줄에 하나씩 설명을 입력하세요." /></label>
        <label>값 <span className="label-hint">선택</span><input type="password" value={value} onChange={(event) => setValue(event.target.value)} placeholder="비워두고 나중에 입력 가능" /></label>
        <div className="modal-actions"><button type="button" className="quiet-button" onClick={onClose}>취소</button><button className="primary-button">변수 추가</button></div>
      </form>
    </Modal>
  );
}

function LinkModal({
  currentFile,
  variable,
  candidates,
  onClose,
  onSubmit,
}: {
  currentFile: string;
  variable: OccurrenceProjection;
  candidates: FileProjection[];
  onClose: () => void;
  onSubmit: (files: string[]) => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(new Set([currentFile]));
  return (
    <Modal
      title={`${variable.key} 연결`}
      description="처음 연결할 때만 현재 파일 값을 기준으로 맞춥니다. 연결 후에는 어느 파일에서 수정해도 모두 함께 저장됩니다."
      onClose={onClose}
    >
      <div className="link-list">
        {candidates.map((candidate) => (
          <label key={candidate.path}>
            <input
              type="checkbox"
              checked={selected.has(candidate.path)}
              disabled={candidate.path === currentFile}
              onChange={(event) => {
                setSelected((current) => {
                  const next = new Set(current);
                  if (event.target.checked) next.add(candidate.path);
                  else next.delete(candidate.path);
                  return next;
                });
              }}
            />
            <span><strong>{candidate.path}</strong><small>{candidate.path === currentFile ? "현재 값 · 기준" : "값은 화면에 노출하지 않고 내부에서 비교"}</small></span>
          </label>
        ))}
      </div>
      <div className="impact-note">
        <strong>{selected.size}개 파일의 {variable.key}</strong>
        <span>연결은 자동으로 만들지 않습니다. 확인하면 선택한 파일들이 하나의 값으로 함께 관리됩니다.</span>
      </div>
      <div className="modal-actions"><button className="quiet-button" onClick={onClose}>취소</button><button className="primary-button" disabled={selected.size < 2} onClick={() => onSubmit([...selected])}>{selected.size}개 occurrence 연결</button></div>
    </Modal>
  );
}
