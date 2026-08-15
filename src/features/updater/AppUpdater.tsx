import { useCallback, useEffect, useState } from "react";

import { localizeError, useI18n } from "../../i18n";
import {
  checkForAppUpdate,
  currentAppVersion,
  installAppUpdate,
  type AppUpdateInfo,
} from "./updateApi";

type UpdateState = "idle" | "checking" | "available" | "installing" | "current" | "error";

export function localizedUpdateNotes(notes: string | null, locale: "en" | "ko") {
  if (!notes) return null;
  const sections = notes
    .replace(/\r\n/g, "\n")
    .split(/^\s*---\s*$/m)
    .map((section) => section.trim())
    .filter(Boolean);
  if (locale === "ko") {
    return sections.find((section) => /[가-힣]/.test(section)) ?? null;
  }
  return sections.find((section) => !/[가-힣]/.test(section)) ?? sections[0] ?? null;
}

export function AppUpdater() {
  const { locale, t } = useI18n();
  const [version, setVersion] = useState("0.6.1");
  const [state, setState] = useState<UpdateState>("idle");
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const updateNotes = localizedUpdateNotes(update?.notes ?? null, locale);

  const runCheck = useCallback(async (manual: boolean) => {
    setState("checking");
    setError(null);
    try {
      const next = await checkForAppUpdate();
      setUpdate(next);
      setState(next ? "available" : manual ? "current" : "idle");
    } catch (reason) {
      setState(manual ? "error" : "idle");
      if (manual) {
        setError(localizeError(reason, locale, "error.updateCheck"));
      }
    }
  }, [locale]);

  useEffect(() => {
    void currentAppVersion().then(setVersion).catch(() => undefined);
    const timer = window.setTimeout(() => void runCheck(false), 1500);
    return () => window.clearTimeout(timer);
  }, [runCheck]);

  const install = async () => {
    setState("installing");
    setError(null);
    try {
      await installAppUpdate();
    } catch (reason) {
      setState("error");
      setError(localizeError(reason, locale, "error.updateInstall"));
    }
  };

  return (
    <>
      <button
        className="version"
        type="button"
        onClick={() => void runCheck(true)}
        disabled={state === "checking" || state === "installing"}
        title={t("updater.check")}
        aria-label={t("updater.label", { version })}
      >
        {state === "checking" ? t("common.checking") : `v${version}`}
        {state === "available" && <span className="update-dot" aria-label={t("updater.available")} />}
      </button>

      {(state === "available" || state === "installing" || state === "error" || state === "current") && (
        <aside className={`update-panel ${state}`} aria-live="polite">
          {state === "available" || state === "installing" ? (
            <>
              <div>
                <span className="update-kicker">{t("updater.availableTitle")}</span>
                <strong>Env Manager {update?.version}</strong>
                {updateNotes && <p>{updateNotes}</p>}
              </div>
              <button type="button" onClick={() => void install()} disabled={state === "installing"}>
                {state === "installing" ? t("common.installing") : t("updater.install")}
              </button>
            </>
          ) : state === "current" ? (
            <>
              <strong>{t("updater.current")}</strong>
              <button type="button" className="update-close" onClick={() => setState("idle")}>{t("common.close")}</button>
            </>
          ) : (
            <>
              <div>
                <strong>{t("updater.failed")}</strong>
                <p>{error}</p>
              </div>
              <button type="button" onClick={() => void runCheck(true)}>{t("updater.retry")}</button>
            </>
          )}
        </aside>
      )}
    </>
  );
}
