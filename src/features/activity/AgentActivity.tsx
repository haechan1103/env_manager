import { useCallback, useEffect, useState } from "react";

import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { AgentActivityEvent } from "../../lib/types";

interface Props {
  projectId: string;
  onError: (message: string) => void;
}

export function AgentActivity({ projectId, onError }: Props) {
  const { locale, t } = useI18n();
  const [events, setEvents] = useState<AgentActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const load = useCallback(async () => {
    setLoading(true);
    try {
      setEvents(await api.listAgentActivity(projectId));
    } catch (error) {
      onError(localizeError(error, locale, "activity.loadError"));
    } finally {
      setLoading(false);
    }
  }, [locale, onError, projectId]);

  useEffect(() => { void load(); }, [load]);

  return (
    <section className="page-stack activity-page">
      <div className="section-heading activity-heading">
        <div>
          <h2>{t("activity.title")}</h2>
          <p>{t("activity.body")}</p>
        </div>
        <button className="quiet-button" onClick={() => void load()}>{t("common.refresh")}</button>
      </div>
      {loading ? (
        <div className="center-state compact"><span className="spinner" /><p>{t("activity.loading")}</p></div>
      ) : events.length === 0 ? (
        <div className="panel empty-inline"><span>✓</span><p>{t("activity.empty")}</p></div>
      ) : (
        <div className="activity-list">
          {events.map((event, index) => (
            <article className="activity-item" key={`${event.timestampMs}:${event.operation}:${index}`}>
              <span className={`activity-outcome ${event.outcome}`} aria-hidden="true" />
              <div className="activity-copy">
                <div className="activity-primary">
                  <strong>{actorName(event.actor)}</strong>
                  <span>{t(categoryKey(event.category))}</span>
                  <time>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(event.timestampMs)}</time>
                </div>
                <p>{event.operation}</p>
                {(event.relativePaths.length > 0 || event.variableNames.length > 0) && (
                  <div className="activity-targets">
                    {event.relativePaths.map((path) => <code key={`p:${path}`}>{path}</code>)}
                    {event.variableNames.map((key) => <code key={`k:${key}`}>{key}</code>)}
                  </div>
                )}
              </div>
              <span className={`activity-result ${event.outcome}`}>{t(outcomeKey(event.outcome))}</span>
            </article>
          ))}
        </div>
      )}
      <p className="audit-footnote">{t("activity.noValues")}</p>
    </section>
  );
}

function actorName(actor: string) {
  return ({ codex: "Codex", "claude-code": "Claude Code", "github-copilot": "GitHub Copilot" } as Record<string, string>)[actor] ?? "AI tool";
}

function categoryKey(category: AgentActivityEvent["category"]) {
  return ({
    "structure-inspection": "activity.category.structure",
    "value-read": "activity.category.read",
    "policy-change": "activity.category.policy",
    mutation: "activity.category.mutation",
  } as const)[category];
}

function outcomeKey(outcome: AgentActivityEvent["outcome"]) {
  return ({
    allowed: "activity.outcome.allowed",
    blocked: "activity.outcome.blocked",
    failed: "activity.outcome.failed",
  } as const)[outcome];
}
