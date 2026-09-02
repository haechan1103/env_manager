import { useEffect, useMemo, useRef, useState } from "react";

import { Modal } from "../../components/Modal";
import { RenameModal } from "../../components/RenameModal";
import { displayGroupName, localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type {
  FileProjection,
  GroupProjection,
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
  onRenameFile: (path: string, name: string) => void;
}

export function FileEditor({
  projectId,
  projection,
  filePath,
  onRefresh,
  onError,
  onNotice,
  onRenameFile,
}: Props) {
  const { locale, t } = useI18n();
  const file = projection.files.find((item) => item.path === filePath);
  const [adding, setAdding] = useState(false);
  const [addingGroup, setAddingGroup] = useState(false);
  const [linking, setLinking] = useState<OccurrenceProjection | null>(null);
  const [migration, setMigration] = useState<MigrationPlanProjection | null>(null);
  const [renamingFile, setRenamingFile] = useState(false);
  const [renamingGroup, setRenamingGroup] = useState<string | null>(null);
  const [showEmptyOnly, setShowEmptyOnly] = useState(false);
  const [activeGroupIndex, setActiveGroupIndex] = useState(0);
  const editorRef = useRef<HTMLElement>(null);

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

  const variableCount = file ? countVariables(file) : 0;
  const emptyVariableCount = file ? countEmptyVariables(file) : 0;
  const visibleGroups = useMemo(() => {
    if (!file) return [];
    if (!showEmptyOnly) return file.groups;
    return file.groups
      .map((group) => ({
        ...group,
        variables: group.variables.filter((variable) => variable.valueState === "empty"),
      }))
      .filter((group) => group.variables.length > 0);
  }, [file, showEmptyOnly]);
  const visibleVariableCount = visibleGroups.reduce(
    (total, group) => total + group.variables.length,
    0,
  );
  const showGroupNavigation = visibleVariableCount >= 10 && visibleGroups.length > 1;

  useEffect(() => {
    setActiveGroupIndex(0);
  }, [filePath, showEmptyOnly]);

  useEffect(() => {
    if (!showGroupNavigation || !editorRef.current) return;
    const scrollRoot = editorRef.current.closest<HTMLElement>(".content-scroll");
    const groupElements = [...editorRef.current.querySelectorAll<HTMLElement>("[data-env-group]")];
    if (!scrollRoot || groupElements.length === 0) return;
    let frame = 0;
    const updateActiveGroup = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const rootTop = scrollRoot.getBoundingClientRect().top;
        const activationLine = rootTop + 86;
        let current = 0;
        for (const [index, element] of groupElements.entries()) {
          if (element.getBoundingClientRect().top <= activationLine) current = index;
        }
        setActiveGroupIndex(current);
      });
    };
    updateActiveGroup();
    scrollRoot.addEventListener("scroll", updateActiveGroup, { passive: true });
    window.addEventListener("resize", updateActiveGroup);
    return () => {
      window.cancelAnimationFrame(frame);
      scrollRoot.removeEventListener("scroll", updateActiveGroup);
      window.removeEventListener("resize", updateActiveGroup);
    };
  }, [filePath, showGroupNavigation, visibleGroups]);

  if (!file) {
    return (
      <section className="center-state">
        <p>{t("file.missing")}</p>
      </section>
    );
  }

  const mutate = async (operation: () => Promise<unknown>, success: string) => {
    try {
      await operation();
      await onRefresh();
      onNotice(success);
    } catch (cause) {
      onError(localizeError(cause, locale, "error.mutation"));
    }
  };

  return (
    <section className="page-stack" ref={editorRef}>
      <div className="file-heading">
        <div>
          <h2>{file.displayName}</h2>
          {file.displayName !== file.path && <code className="file-physical-path">{file.path}</code>}
          <p>{t("file.summary", { variables: variableCount, groups: file.groups.length })}</p>
        </div>
        <div className="header-actions">
          <button
            className="quiet-button"
            aria-label={t("file.renameFile")}
            onClick={() => setRenamingFile(true)}
          >
            {t("common.rename")}
          </button>
          <button
            className="quiet-button"
            onClick={() => {
              void api
                .planMigration(projectId, file.path)
                .then(setMigration)
                .catch((cause: unknown) =>
                  onError(
                    localizeError(cause, locale, "error.migration"),
                  ),
                );
            }}
          >
            {t("file.organizeComments")}
          </button>
          <button className="quiet-button" onClick={() => setAddingGroup(true)}>{t("file.newGroup")}</button>
          <button className="primary-button" onClick={() => setAdding(true)}>{t("file.newVariable")}</button>
        </div>
      </div>

      {file.warnings.length > 0 && (
        <div className="warning-banner">
          <strong>{t("file.warningTitle")}</strong>
          <span>{localizedWarnings(file.warnings, locale, t)}</span>
        </div>
      )}

      {variableCount > 0 && (
        <div className="file-filter-bar" role="toolbar" aria-label={t("file.filters")}>
          <button
            className={`file-filter-toggle${showEmptyOnly ? " active" : ""}`}
            aria-pressed={showEmptyOnly}
            onClick={() => setShowEmptyOnly((current) => !current)}
          >
            <span>{t("file.emptyOnly")}</span>
            <strong aria-label={t("file.emptyCount", { count: emptyVariableCount })}>
              {emptyVariableCount}
            </strong>
          </button>
          {showEmptyOnly && (
            <span className="file-filter-result">
              {t("file.filteredCount", { visible: visibleVariableCount, total: variableCount })}
            </span>
          )}
        </div>
      )}

      {showGroupNavigation && (
        <GroupJumpNavigation
          groups={visibleGroups}
          activeGroupIndex={activeGroupIndex}
          onJump={(groupIndex) => {
            setActiveGroupIndex(groupIndex);
            editorRef.current
              ?.querySelector<HTMLElement>(`[data-env-group="${groupIndex}"]`)
              ?.scrollIntoView({ behavior: "smooth", block: "start" });
          }}
        />
      )}

      <div className="groups-stack">
        {showEmptyOnly && visibleGroups.length === 0 && (
          <div className="file-filter-empty">
            <strong>{t("file.noEmptyVariables")}</strong>
            <span>{t("file.noEmptyVariablesBody")}</span>
          </div>
        )}
        {visibleGroups.map((group, groupIndex) => (
          <section
            className="group-card"
            data-env-group={groupIndex}
            key={`${group.name}:${groupIndex}`}
          >
            <header className="group-header">
              <div>
                <span className="group-fold">⌄</span>
                <h3>{displayGroupName(group.name, t)}</h3>
                <span>{group.variables.length}</span>
              </div>
              {group.name !== "기타" && (
                <button
                  className="quiet-button compact"
                  aria-label={t("file.renameGroupNamed", { name: displayGroupName(group.name, t) })}
                  onClick={() => setRenamingGroup(group.name)}
                >
                  {t("file.renameGroup")}
                </button>
              )}
            </header>
            <div className="variables-table">
              {group.variables.length === 0 && (
                <div className="empty-group-row">{t("file.emptyGroup")}</div>
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
              t("file.variableAdded", { key: request.key }),
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
              t("file.groupCreated", { name }),
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
              t("file.variablesLinked", { key: linking.key, count: files.length }),
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
              t("file.migrationApplied", { file: migration.preview.file }),
            ).then(() => setMigration(null));
          }}
        />
      )}
      {renamingFile && (
        <RenameModal
          title={t("sidebar.fileNamePrompt")}
          currentName={file.displayName}
          onClose={() => setRenamingFile(false)}
          onRename={(name) => onRenameFile(file.path, name)}
        />
      )}
      {renamingGroup && (
        <RenameModal
          title={t("file.renameGroupPrompt")}
          currentName={renamingGroup}
          onClose={() => setRenamingGroup(null)}
          onRename={(name) => {
            const currentName = renamingGroup;
            void mutate(
              () => api.renameGroup(projectId, {
                file: file.path,
                currentName,
                newName: name,
              }),
              t("file.groupRenamed"),
            );
          }}
        />
      )}
    </section>
  );
}

function GroupJumpNavigation({
  groups,
  activeGroupIndex,
  onJump,
}: {
  groups: GroupProjection[];
  activeGroupIndex: number;
  onJump: (index: number) => void;
}) {
  const { t } = useI18n();
  const trackRef = useRef<HTMLDivElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const [expanded, setExpanded] = useState(false);

  const updateOverflow = () => {
    const track = trackRef.current;
    if (!track) return;
    setCanScrollLeft(track.scrollLeft > 2);
    setCanScrollRight(track.scrollLeft + track.clientWidth < track.scrollWidth - 2);
  };

  useEffect(() => {
    const track = trackRef.current;
    if (!track) return;
    updateOverflow();
    const resizeObserver = typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(updateOverflow);
    resizeObserver?.observe(track);
    window.addEventListener("resize", updateOverflow);
    return () => {
      resizeObserver?.disconnect();
      window.removeEventListener("resize", updateOverflow);
    };
  }, [groups]);

  useEffect(() => {
    if (expanded) return;
    const track = trackRef.current;
    const active = track?.querySelector<HTMLElement>(`[data-group-shortcut="${activeGroupIndex}"]`);
    if (!track || !active) return;
    const left = active.offsetLeft;
    const right = left + active.offsetWidth;
    const visibleLeft = track.scrollLeft;
    const visibleRight = visibleLeft + track.clientWidth;
    if (left < visibleLeft || right > visibleRight) {
      track.scrollTo({
        left: Math.max(0, left - (track.clientWidth - active.offsetWidth) / 2),
        behavior: "smooth",
      });
    }
  }, [activeGroupIndex, expanded]);

  const moveTrack = (direction: -1 | 1) => {
    const track = trackRef.current;
    if (!track) return;
    track.scrollBy({ left: direction * Math.max(160, track.clientWidth * 0.7), behavior: "smooth" });
  };

  return (
    <nav className={`group-jump-nav${expanded ? " expanded" : ""}`} aria-label={t("file.groupNavigation")}>
      {!expanded && (
        <button
          className="group-jump-arrow"
          aria-label={t("file.previousGroups")}
          disabled={!canScrollLeft}
          onClick={() => moveTrack(-1)}
        >‹</button>
      )}
      <div className={`group-jump-window${!expanded && canScrollLeft ? " can-scroll-left" : ""}${!expanded && canScrollRight ? " can-scroll-right" : ""}`}>
        <div className="group-jump-track" ref={trackRef} onScroll={updateOverflow}>
          {groups.map((group, groupIndex) => (
            <button
              key={`${group.name}:${groupIndex}`}
              data-group-shortcut={groupIndex}
              className={activeGroupIndex === groupIndex ? "active" : undefined}
              aria-label={`${displayGroupName(group.name, t)} · ${group.variables.length}`}
              aria-current={activeGroupIndex === groupIndex ? "location" : undefined}
              onClick={() => onJump(groupIndex)}
            >
              {displayGroupName(group.name, t)}
              <small>{group.variables.length}</small>
            </button>
          ))}
        </div>
      </div>
      {!expanded && (
        <button
          className="group-jump-arrow"
          aria-label={t("file.nextGroups")}
          disabled={!canScrollRight}
          onClick={() => moveTrack(1)}
        >›</button>
      )}
      <button
        className="group-jump-all"
        aria-label={expanded ? t("file.collapseGroups") : t("file.allGroups")}
        title={expanded ? t("file.collapseGroups") : t("file.allGroups")}
        aria-expanded={expanded}
        onClick={() => setExpanded((open) => !open)}
      >
        {expanded ? (
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d="m5 12 5-5 5 5" />
          </svg>
        ) : (
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <rect x="3.5" y="3.5" width="5" height="5" rx="1" />
            <rect x="11.5" y="3.5" width="5" height="5" rx="1" />
            <rect x="3.5" y="11.5" width="5" height="5" rx="1" />
            <rect x="11.5" y="11.5" width="5" height="5" rx="1" />
          </svg>
        )}
      </button>
    </nav>
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
  const { t } = useI18n();
  return (
    <Modal
      title={t("migration.title")}
      description={t("migration.validFor", { file: plan.preview.file, minutes: plan.expiresInSeconds / 60 })}
      onClose={onClose}
    >
      <div className="migration-summary">
        <p>{t("migration.summary", { count: plan.preview.suggestions.length })}</p>
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
          <strong>{t("migration.noValues")}</strong>
          <span>{t("migration.changedGuard")}</span>
        </div>
      </div>
      <div className="modal-actions">
        <button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
        <button className="primary-button" onClick={onApply}>{t("migration.apply")}</button>
      </div>
    </Modal>
  );
}

function countVariables(file: FileProjection) {
  return file.groups.reduce((total, group) => total + group.variables.length, 0);
}

function countEmptyVariables(file: FileProjection) {
  return file.groups.reduce(
    (total, group) =>
      total + group.variables.filter((variable) => variable.valueState === "empty").length,
    0,
  );
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
  const { t } = useI18n();
  const [name, setName] = useState("");
  return (
    <Modal
      title={t("group.newTitle")}
      description={t("group.newDescription", { file })}
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
          {t("group.name")}
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="GPT"
            autoFocus
          />
        </label>
        <div className="impact-note">
          <strong>{t("group.preserveTitle")}</strong>
          <span>{t("group.preserveBody")}</span>
        </div>
        <div className="modal-actions">
          <button type="button" className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
          <button className="primary-button" disabled={!name.trim()}>{t("group.create")}</button>
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
  const { t } = useI18n();
  const [key, setKey] = useState("");
  const [group, setGroup] = useState(file.groups[0]?.name ?? "기타");
  const [newGroup, setNewGroup] = useState("");
  const [description, setDescription] = useState("");
  const [value, setValue] = useState("");
  return (
    <Modal
      title={t("variable.newTitle")}
      description={t("variable.newDescription", { file: file.path })}
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
        <label>{t("variable.name")}<input value={key} onChange={(event) => setKey(event.target.value)} placeholder="NEW_VARIABLE" autoFocus /></label>
        <label>{t("variable.group")}
          <select value={group} onChange={(event) => setGroup(event.target.value)}>
            {file.groups.map((item) => <option key={item.name} value={item.name}>{displayGroupName(item.name, t)}</option>)}
            {!file.groups.some((item) => item.name === "기타") && <option value="기타">{t("group.ungroupedOption")}</option>}
            <option value="__new_group__">{t("variable.createGroup")}</option>
          </select>
        </label>
        {group === "__new_group__" && (
          <label>{t("variable.newGroupName")}<input value={newGroup} onChange={(event) => setNewGroup(event.target.value)} placeholder="GPT" /></label>
        )}
        <label>{t("variable.description")}<textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder={t("variable.descriptionPlaceholder")} /></label>
        <label>{t("variable.value")} <span className="label-hint">{t("common.optional")}</span><input type="password" value={value} onChange={(event) => setValue(event.target.value)} placeholder={t("variable.valueLater")} /></label>
        <div className="modal-actions"><button type="button" className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button">{t("variable.add")}</button></div>
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
  const { t } = useI18n();
  const [selected, setSelected] = useState<Set<string>>(new Set([currentFile]));
  return (
    <Modal
      title={t("link.title", { key: variable.key })}
      description={t("link.description")}
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
            <span><strong>{candidate.path}</strong><small>{candidate.path === currentFile ? t("link.source") : t("link.compare")}</small></span>
          </label>
        ))}
      </div>
      <div className="impact-note">
        <strong>{t("link.selection", { key: variable.key, count: selected.size })}</strong>
        <span>{t("link.confirmation")}</span>
      </div>
      <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={selected.size < 2} onClick={() => onSubmit([...selected])}>{t("link.submit", { count: selected.size })}</button></div>
    </Modal>
  );
}

function localizedWarnings(
  warnings: string[],
  locale: "en" | "ko",
  t: ReturnType<typeof useI18n>["t"],
) {
  if (locale === "ko") return warnings.join(" ");
  const translated = warnings.map((warning) => {
    if (warning.includes("알 수 없는 Kavranta 지시문")) {
      return t("file.warningUnknownDirective");
    }
    if (warning.includes("해석하지 못한 줄")) return t("file.warningUnknownLine");
    return null;
  });
  return translated.every(Boolean)
    ? translated.join(" ")
    : t("file.warningCount", { count: warnings.length });
}
