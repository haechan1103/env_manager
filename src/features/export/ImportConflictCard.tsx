import { useEffect, useState } from "react";

import { useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { TeamImportValueSide } from "../../lib/types";

export interface ImportConflictOccurrence {
  id: string;
  key: string;
  sourcePath: string;
  targetPath: string;
  linkId: string | null;
}

interface Props {
  projectId: string;
  planId: string;
  occurrences: ImportConflictOccurrence[];
  useShared: boolean;
  onChoice: (useShared: boolean) => void;
  onError: (error: unknown) => void;
}

export function ImportConflictCard({
  projectId,
  planId,
  occurrences,
  useShared,
  onChoice,
  onError,
}: Props) {
  const { t } = useI18n();
  const [revealed, setRevealed] = useState<Partial<Record<TeamImportValueSide, string>>>({});
  const [activity, setActivity] = useState(0);
  const representative = occurrences[0];

  useEffect(() => {
    if (Object.keys(revealed).length === 0) return;
    const timeout = window.setTimeout(() => setRevealed({}), 30000);
    return () => window.clearTimeout(timeout);
  }, [activity, revealed]);

  if (!representative) return null;

  const toggleReveal = async (side: TeamImportValueSide) => {
    if (revealed[side] !== undefined) {
      setRevealed((current) => {
        const next = { ...current };
        delete next[side];
        return next;
      });
      return;
    }
    try {
      const value = await api.revealTeamImportConflict(
        projectId,
        planId,
        representative.id,
        side,
      );
      setRevealed((current) => ({ ...current, [side]: value }));
      setActivity((current) => current + 1);
    } catch (error) {
      onError(error);
    }
  };

  const keepVisible = () => {
    if (Object.keys(revealed).length > 0) setActivity((current) => current + 1);
  };

  return (
    <article className="import-conflict-card" onFocus={keepVisible} onPointerDown={keepVisible} onKeyDown={keepVisible}>
      <header>
        <div>
          <code>{representative.key}</code>
          {representative.linkId && <span className="import-linked-badge">{t("import.linkedCount", { count: occurrences.length })}</span>}
        </div>
        <div className="import-conflict-decision" role="group" aria-label={t("import.choiceFor", { key: representative.key })}>
          <button className={!useShared ? "selected" : ""} onClick={() => onChoice(false)}>{t("import.keepLocal")}</button>
          <button className={useShared ? "selected" : ""} onClick={() => onChoice(true)}>{t("import.useShared")}</button>
        </div>
      </header>

      <div className="import-conflict-values">
        {(["local", "shared"] as TeamImportValueSide[]).map((side) => {
          const value = revealed[side];
          return (
            <section key={side} className={value !== undefined ? "revealed" : ""}>
              <header>
                <strong>{side === "local" ? t("import.localValue") : t("import.sharedValue")}</strong>
                <button
                  className="icon-button"
                  aria-label={value === undefined
                    ? side === "local" ? t("import.revealLocal") : t("import.revealShared")
                    : t("row.hide")}
                  title={value === undefined ? t("import.revealFor30Seconds") : t("row.hide")}
                  onClick={() => void toggleReveal(side)}
                >
                  {value === undefined ? "◉" : "○"}
                </button>
              </header>
              {value === undefined ? <span className="import-masked-value">••••••••</span> : <pre>{value || t("row.valueEmpty")}</pre>}
            </section>
          );
        })}
      </div>

      <ul className="import-conflict-paths">
        {occurrences.map((occurrence) => (
          <li key={occurrence.id}>
            <code>{occurrence.sourcePath}</code>
            {occurrence.sourcePath !== occurrence.targetPath && <><span>→</span><code>{occurrence.targetPath}</code></>}
          </li>
        ))}
      </ul>
      {representative.linkId && <p className="linked-share-note">{t("import.linkedChoiceHelp")}</p>}
    </article>
  );
}
