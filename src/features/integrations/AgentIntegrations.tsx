import { useCallback, useEffect, useState } from "react";

import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type {
  AgentIntegrationId,
  AgentIntegrationStatus,
} from "../../lib/types";
import { useAgentIntegrationStatus } from "./AgentIntegrationStatusProvider";

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
  const { locale, t } = useI18n();
  const { items, loading, refresh, replace } = useAgentIntegrationStatus();
  const [installing, setInstalling] = useState<AgentIntegrationId | null>(null);

  const load = useCallback(async () => {
    try {
      await refresh();
    } catch (error) {
      onError(localizeError(error, locale, "error.integrationStatus"));
    }
  }, [locale, onError, refresh]);

  useEffect(() => {
    void load();
  }, [load]);

  const install = async (item: AgentIntegrationStatus) => {
    setInstalling(item.id);
    try {
      const result = await api.installAgentIntegration(item.id);
      replace(result);
      onNotice(t("integration.installSuccess", { name: item.name, version: result.currentVersion }));
    } catch (error) {
      onError(localizeError(error, locale, "error.integrationInstall", { name: item.name }));
    } finally {
      setInstalling(null);
    }
  };

  return (
    <section className="integration-page">
      <div className="integration-intro">
        <div>
          <h2>{t("integration.heading")}</h2>
          <p>{t("integration.body")}</p>
        </div>
        <button className="quiet-button" onClick={() => void load()} disabled={loading}>
          {loading ? t("common.checking") : t("integration.refresh")}
        </button>
      </div>

      <div className="integration-grid" aria-live="polite">
        {items.map((item) => {
          const busy = installing === item.id;
          const actionNeeded = !item.installed || item.updateAvailable || item.needsRepair;
          const actionLabel = item.updateAvailable
            ? t("integration.update")
            : item.needsRepair
              ? t("integration.repair")
              : item.installed
                ? t("integration.installed")
                : t("integration.install");
          const connected = item.installed && !item.needsRepair;
          return (
            <article className={`integration-card ${connected ? "connected" : ""}`} key={item.id}>
              <header>
                <span className={`integration-mark ${item.id}`}>{marks[item.id]}</span>
                <div>
                  <h3>{item.name}</h3>
                  <span className={`integration-state ${connected ? "installed" : item.detected ? "detected" : "missing"}`}>
                    {item.needsRepair ? t("integration.repairNeeded") : connected ? t("integration.connected") : item.detected ? t("integration.detected") : t("integration.missing")}
                  </span>
                </div>
              </header>

              <p className="integration-detail">{integrationDetail(item, t)}</p>

              <dl className="integration-meta">
                <div>
                  <dt>{t("integration.version")}</dt>
                  <dd>
                    {item.legacyVersion
                      ? t("integration.legacyVersion", { version: item.installedVersion ?? "—" })
                      : item.installedVersion ?? "—"}
                    {item.updateAvailable ? ` → ${item.currentVersion}` : ""}
                  </dd>
                </div>
                <div>
                  <dt>{t("integration.protection")}</dt>
                  <dd>{protectionLabel(item.protection, t)}</dd>
                </div>
              </dl>

              <button
                className={connected ? "quiet-button integration-action" : "primary-button integration-action"}
                disabled={busy || !actionNeeded || !item.canInstall}
                onClick={() => void install(item)}
              >
                {busy ? t("common.installing") : actionLabel}
              </button>
              {actionNeeded && item.actionBlocker && (
                <p className="integration-action-hint">{actionBlockerLabel(item, t)}</p>
              )}
            </article>
          );
        })}
        {loading && items.length === 0 && <div className="integration-loading">{t("integration.loading")}</div>}
      </div>

      <div className="integration-footnote">
        <span>i</span>
        <p>{t("integration.footnote")}</p>
      </div>
    </section>
  );
}

function protectionLabel(
  protection: AgentIntegrationStatus["protection"],
  t: ReturnType<typeof useI18n>["t"],
) {
  if (protection === "broker") return t("integration.protectionBroker");
  if (protection === "guarded") return t("integration.protectionGuarded");
  return t("integration.protectionInactive");
}

function integrationDetail(
  item: AgentIntegrationStatus,
  t: ReturnType<typeof useI18n>["t"],
) {
  if (item.needsRepair) return t("integration.detailRepair");
  if (item.installed && item.id === "codex") return t("integration.detailCodex");
  if (item.installed) return t("integration.detailGuarded");
  if (item.id === "github-copilot" && item.detected && !item.canInstall) {
    return t("integration.detailCopilotCli");
  }
  if (item.detected) return t("integration.detailDetected");
  return t("integration.detailMissing");
}

function actionBlockerLabel(
  item: AgentIntegrationStatus,
  t: ReturnType<typeof useI18n>["t"],
) {
  if (item.actionBlocker === "broker-unavailable") {
    return t("integration.blockerBroker");
  }
  if (item.actionBlocker === "bundle-unavailable") {
    return t("integration.blockerBundle");
  }
  return t("integration.blockerTool", { name: item.name });
}
