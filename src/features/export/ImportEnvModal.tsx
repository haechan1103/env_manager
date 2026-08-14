import { useEffect, useMemo, useRef, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { ProjectProjection, TeamImportPlanProjection } from "../../lib/types";
import { ImportConflictCard, type ImportConflictOccurrence } from "./ImportConflictCard";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onApplied: () => Promise<void>;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

interface ConflictGroup {
  id: string;
  occurrences: ImportConflictOccurrence[];
}

interface ImportTargetRowProps {
  sourcePath: string;
  targetPath: string;
  usedTargets: Set<string>;
  busy: boolean;
  onRemap: (targetPath: string) => void;
}

function ImportTargetRow({ sourcePath, targetPath, usedTargets, busy, onRemap }: ImportTargetRowProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(targetPath);

  useEffect(() => setDraft(targetPath), [targetPath]);

  const normalized = draft.trim();
  const changed = normalized !== targetPath;
  return (
    <div className="import-target-row">
      <span><code>{sourcePath}</code><small>{t("import.incomingFile")}</small></span>
      <span aria-hidden="true">→</span>
      <input
        aria-label={t("import.targetFor", { file: sourcePath })}
        list="import-target-options"
        value={draft}
        disabled={busy}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && changed && normalized && !usedTargets.has(normalized)) {
            event.preventDefault();
            onRemap(normalized);
          }
        }}
      />
      <button
        className="quiet-button compact"
        disabled={busy || !changed || !normalized || usedTargets.has(normalized)}
        onClick={() => onRemap(normalized)}
      >
        {busy ? t("import.remapping") : t("import.changeTarget")}
      </button>
    </div>
  );
}

export function ImportEnvModal({ projectId, projection, onApplied, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [passphrase, setPassphrase] = useState("");
  const [plan, setPlan] = useState<TeamImportPlanProjection | null>(null);
  const [sharedConflicts, setSharedConflicts] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [remappingFile, setRemappingFile] = useState<string | null>(null);
  const planIdRef = useRef<string | null>(null);

  useEffect(() => {
    planIdRef.current = plan?.planId ?? null;
  }, [plan?.planId]);

  useEffect(() => () => {
    const planId = planIdRef.current;
    if (planId) void api.discardTeamImport(projectId, planId);
  }, [projectId]);

  const conflictGroups = useMemo<ConflictGroup[]>(() => {
    if (!plan) return [];
    const groups = new Map<string, ImportConflictOccurrence[]>();
    for (const file of plan.preview.files) {
      for (const occurrence of file.occurrences) {
        if (occurrence.state !== "conflict") continue;
        const groupId = occurrence.linkId ? `link:${occurrence.linkId}` : `occurrence:${occurrence.id}`;
        const items = groups.get(groupId) ?? [];
        items.push({
          id: occurrence.id,
          key: occurrence.key,
          sourcePath: file.path,
          targetPath: file.targetPath,
          linkId: occurrence.linkId,
        });
        groups.set(groupId, items);
      }
    }
    return [...groups.entries()].map(([id, occurrences]) => ({ id, occurrences }));
  }, [plan]);

  const targetOptions = useMemo(() => {
    const paths = new Set(projection.files.map((file) => file.path));
    for (const file of plan?.preview.files ?? []) paths.add(file.path);
    return [...paths].sort((left, right) => left.localeCompare(right));
  }, [plan?.preview.files, projection.files]);

  const choose = async () => {
    if (passphrase.length < 10) return;
    setBusy(true);
    try {
      const result = await api.planTeamImport(projectId, passphrase, locale);
      setPassphrase("");
      if (result) {
        setPlan(result);
        setSharedConflicts(new Set());
      }
    } catch (error) {
      setPassphrase("");
      onError(localizeError(error, locale, "import.error"));
    } finally {
      setBusy(false);
    }
  };

  const chooseGroup = (occurrences: ImportConflictOccurrence[], useShared: boolean) => {
    setSharedConflicts((previous) => {
      const next = new Set(previous);
      for (const occurrence of occurrences) {
        if (useShared) next.add(occurrence.id);
        else next.delete(occurrence.id);
      }
      return next;
    });
  };

  const remapFile = async (sourceFile: string, targetFile: string) => {
    if (!plan) return;
    setRemappingFile(sourceFile);
    try {
      const preview = await api.remapTeamImportFile(projectId, plan.planId, sourceFile, targetFile);
      setPlan((current) => current ? { ...current, preview } : current);
      setSharedConflicts(new Set());
    } catch (error) {
      onError(localizeError(error, locale, "import.remapError"));
    } finally {
      setRemappingFile(null);
    }
  };

  const apply = async () => {
    if (!plan) return;
    setBusy(true);
    try {
      const result = await api.applyTeamImport(projectId, plan.planId, [...sharedConflicts]);
      planIdRef.current = null;
      setPlan(null);
      await onApplied();
      onNotice(t("import.done", {
        added: result.addedCount,
        updated: result.updatedCount,
        kept: result.keptLocalCount,
        unchanged: result.unchangedCount,
      }));
      onClose();
    } catch (error) {
      onError(localizeError(error, locale, "import.error"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal className="import-modal" title={t("import.title")} description={t("import.description")} onClose={onClose}>
      {!plan ? (
        <>
          <div className="modal-form import-password-field">
            <label><span>{t("import.passphrase")}</span><input type="password" autoComplete="off" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} /></label>
            <p className="field-help">{t("import.passphraseHelp")}</p>
          </div>
          <div className="export-warning"><strong>{t("import.warningTitle")}</strong><p>{t("import.warningBody")}</p></div>
          <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={busy || passphrase.length < 10} onClick={() => void choose()}>{busy ? t("import.opening") : t("import.choose")}</button></div>
        </>
      ) : (
        <>
          <section className="import-target-section">
            <header><strong>{t("import.targetTitle")}</strong><small>{t("import.targetHelp")}</small></header>
            <div className="import-target-list">
              {plan.preview.files.map((file) => {
                const usedTargets = new Set(plan.preview.files.filter((item) => item.path !== file.path).map((item) => item.targetPath));
                return (
                  <ImportTargetRow
                    key={file.path}
                    sourcePath={file.path}
                    targetPath={file.targetPath}
                    usedTargets={usedTargets}
                    busy={remappingFile !== null}
                    onRemap={(targetPath) => void remapFile(file.path, targetPath)}
                  />
                );
              })}
              <datalist id="import-target-options">
                {targetOptions.map((path) => <option key={path} value={path} />)}
              </datalist>
            </div>
          </section>

          <div className="import-summary" aria-live="polite">
            <span><strong>{plan.preview.newCount}</strong>{t("import.new")}</span>
            <span><strong>{plan.preview.unchangedCount}</strong>{t("import.unchanged")}</span>
            <span className={plan.preview.conflictCount > 0 ? "conflict" : undefined}><strong>{plan.preview.conflictCount}</strong>{t("import.conflict")}</span>
          </div>

          {conflictGroups.length > 0 ? (
            <section className="import-conflict-section">
              <header>
                <div><strong>{t("import.conflictTitle")}</strong><small>{t("import.conflictBody", { count: plan.preview.conflictCount })}</small></div>
                <div className="import-batch-actions">
                  <button className="text-button" onClick={() => setSharedConflicts(new Set())}>{t("import.keepAllLocal")}</button>
                  <button className="text-button" onClick={() => setSharedConflicts(new Set(conflictGroups.flatMap((group) => group.occurrences.map((item) => item.id))))}>{t("import.useAllShared")}</button>
                </div>
              </header>
              <div className="import-conflict-list">
                {conflictGroups.map((group) => (
                  <ImportConflictCard
                    key={group.id}
                    projectId={projectId}
                    planId={plan.planId}
                    occurrences={group.occurrences}
                    useShared={group.occurrences.every((item) => sharedConflicts.has(item.id))}
                    onChoice={(useShared) => chooseGroup(group.occurrences, useShared)}
                    onError={(error) => onError(localizeError(error, locale, "import.revealError"))}
                  />
                ))}
              </div>
            </section>
          ) : (
            <div className="import-no-conflicts"><strong>{t("import.noConflicts")}</strong><p>{t("import.noConflictsBody")}</p></div>
          )}

          <section className="import-final-summary">
            <strong>{t("import.finalTitle")}</strong>
            <div>
              <span>{t("import.finalAdded", { count: plan.preview.newCount })}</span>
              <span>{t("import.finalChanged", { count: sharedConflicts.size })}</span>
              <span>{t("import.finalKept", { count: plan.preview.conflictCount - sharedConflicts.size })}</span>
              <span>{t("import.finalUnchanged", { count: plan.preview.unchangedCount })}</span>
            </div>
          </section>
          <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.cancel")}</button><button className="primary-button" disabled={busy || remappingFile !== null} onClick={() => void apply()}>{busy ? t("import.applying") : t("import.apply")}</button></div>
        </>
      )}
    </Modal>
  );
}
