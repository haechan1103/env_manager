import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { displayGroupName, useI18n } from "../../i18n";
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
  const { t } = useI18n();
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
    variable.valueState === "present" ? t("row.valuePresent") : t("row.valueEmpty");
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
        t("row.accessConfirm", { key: variable.key }),
      )
    ) {
      return;
    }
    await onMutate(
      () => api.setCodexAccess(projectId, variable.key, access, downgrade),
      t("row.accessChanged", { key: variable.key }),
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
              aria-label={t("row.copyKeyLabel", { key: variable.key })}
              title={keyCopied ? t("common.copied") : t("row.copyKey")}
              onClick={() => {
                void api
                  .copyKey(projectId, variable.key)
                  .then(() => setKeyCopied(true));
              }}
            >
              {keyCopied ? "✓" : "⧉"}
            </button>
            {variable.linkedCount > 1 && (
              <span className="badge linked">{t("row.filesLinked", { count: variable.linkedCount })}</span>
            )}
            {hasIndependentPeers && (
              <span className="badge available">{t("row.sameVariable", { count: sameKeyFiles.length })}</span>
            )}
            {variable.duplicate && <span className="badge error">{t("row.duplicate")}</span>}
          </div>
          {variable.description.length > 0 ? (
            <button className="description-button" onClick={() => setEditingDescription((open) => !open)}>
              {variable.description.join(" ")}
            </button>
          ) : (
            <button className="description-button muted" onClick={() => setEditingDescription(true)}>{t("row.addDescription")}</button>
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
              aria-label={t("row.valueLabel", { key: variable.key })}
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
              aria-label={t("row.valueLabel", { key: variable.key })}
              onChange={(event) => {
                setDraft(event.target.value);
                setDirty(true);
              }}
            />
          )}
          <button
            className="icon-button"
            title={revealed === null ? t("row.reveal") : t("row.hide")}
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
            title={t("row.copyValue")}
            onClick={() => void api.copyValue(projectId, file, variable.key)}
          >
            ⧉
          </button>
        </div>

        <div className="variable-actions">
          <select
            className={`access-select ${variable.codexAccess}`}
            value={variable.codexAccess}
            aria-label={t("row.aiAccess", { key: variable.key })}
            onChange={(event) => void changeAccess(event.target.value as CodexAccess)}
          >
            <option value="protected">{t("row.protected")}</option>
            <option value="unclassified">{t("row.unclassified")}</option>
            <option value="read-write">{t("row.readWrite")}</option>
          </select>
          {dirty ? (
            <button
              className="primary-button compact"
              disabled={variable.duplicate}
              onClick={() =>
                void onMutate(
                  () => api.saveValue(projectId, { file, key: variable.key, newValue: draft }),
                  variable.linkedCount > 1
                    ? t("row.savedLinked", { count: variable.linkedCount })
                    : t("row.saved", { key: variable.key }),
                ).then(() => {
                  setDirty(false);
                  setDraft("");
                })
              }
            >
              {variable.linkedCount > 1 ? t("row.saveLinked", { count: variable.linkedCount }) : t("common.save")}
            </button>
          ) : null}
          <button
            className="quiet-button compact"
            title={t("row.moveTitle")}
            onClick={() => {
              const choices = groups.filter((group) => group !== currentGroup);
              if (choices.length === 0) return;
              const displayedChoices = choices.map((group) => displayGroupName(group, t));
              const selected = window
                .prompt(
                  t("row.movePrompt", { groups: displayedChoices.join(" · ") }),
                  displayedChoices[0],
                )
                ?.trim();
              const target = choices.find(
                (group) => group === selected || displayGroupName(group, t) === selected,
              );
              if (!target) return;
              void onMutate(
                () =>
                  api.moveVariable(projectId, {
                    file,
                    key: variable.key,
                    targetGroup: target,
                  }),
                t("row.moved", { key: variable.key, group: displayGroupName(target, t) }),
              );
            }}
          >
            {t("common.move")}
          </button>
          <button
            className="danger-quiet-button compact"
            title={t("row.deleteTitle")}
            onClick={() => {
              if (
                window.confirm(
                  t("row.deleteConfirm", { file, key: variable.key }),
                )
              ) {
                void onMutate(
                  () => api.deleteVariable(projectId, { file, key: variable.key }),
                  t("row.deleted", { key: variable.key }),
                );
              }
            }}
          >
            {t("common.delete")}
          </button>
        </div>
      </div>

      {variable.linkId && variable.linkedFiles.length > 1 && (
        <div className="relationship-panel linked-relationship">
          <span className="relationship-icon" aria-hidden="true">↔</span>
          <div className="relationship-copy">
            <strong>{t("row.linkedTitle", { count: variable.linkedFiles.length })}</strong>
            <span>{t("row.linkedBody")}</span>
            <div className="relationship-paths">
              {variable.linkedFiles.map((path) => (
                <code className={path === file ? "current" : ""} key={path}>
                  {path}
                  {path === file && <small>{t("common.current")}</small>}
                </code>
              ))}
            </div>
            {unlinkedPeerFiles.length > 0 && (
              <span className="unlinked-peer-note">
                {t("row.separate", { files: unlinkedPeerFiles.join(" · ") })}
              </span>
            )}
          </div>
          <button
            className="quiet-button compact relationship-action"
            onClick={() => {
              if (window.confirm(t("row.detachConfirm"))) {
                void onMutate(
                  () => api.detachLink(projectId, variable.linkId!, file),
                  t("row.detached", { file }),
                );
              }
            }}
          >
            {t("row.detach")}
          </button>
        </div>
      )}

      {hasIndependentPeers && (
        <div className="relationship-panel available-relationship">
          <span className="relationship-icon" aria-hidden="true">＋</span>
          <div className="relationship-copy">
            <strong>{t("row.peersTitle", { count: sameKeyFiles.length })}</strong>
            <span>{t("row.peersBody")}</span>
            <div className="relationship-paths">
              {sameKeyFiles.map((path) => (
                <code className={path === file ? "current" : ""} key={path}>
                  {path}
                  {path === file && <small>{t("common.current")}</small>}
                </code>
              ))}
            </div>
          </div>
          <button className="secondary-button compact relationship-action" onClick={onLink}>
            {t("row.manageTogether")}
          </button>
        </div>
      )}

      {editingDescription && (
        <div className="description-editor">
          <textarea value={description} onChange={(event) => setDescription(event.target.value)} />
          <div>
            <button className="quiet-button compact" onClick={() => setEditingDescription(false)}>{t("common.cancel")}</button>
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
                  t("row.descriptionSaved", { key: variable.key }),
                ).then(() => setEditingDescription(false))
              }
            >
              {t("row.saveDescription")}
            </button>
          </div>
        </div>
      )}
    </article>
  );
}
