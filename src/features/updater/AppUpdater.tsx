import { useCallback, useEffect, useState } from "react";

import {
  checkForAppUpdate,
  currentAppVersion,
  installAppUpdate,
  type AppUpdateInfo,
} from "./updateApi";

type UpdateState = "idle" | "checking" | "available" | "installing" | "current" | "error";

export function AppUpdater() {
  const [version, setVersion] = useState("0.3.0");
  const [state, setState] = useState<UpdateState>("idle");
  const [update, setUpdate] = useState<AppUpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

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
        setError(reason instanceof Error ? reason.message : "업데이트를 확인하지 못했습니다.");
      }
    }
  }, []);

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
      setError(reason instanceof Error ? reason.message : "업데이트를 설치하지 못했습니다.");
    }
  };

  return (
    <>
      <button
        className="version"
        type="button"
        onClick={() => void runCheck(true)}
        disabled={state === "checking" || state === "installing"}
        title="업데이트 확인"
        aria-label={`Env Manager v${version}, 업데이트 확인`}
      >
        {state === "checking" ? "확인 중…" : `v${version}`}
        {state === "available" && <span className="update-dot" aria-label="업데이트 있음" />}
      </button>

      {(state === "available" || state === "installing" || state === "error" || state === "current") && (
        <aside className={`update-panel ${state}`} aria-live="polite">
          {state === "available" || state === "installing" ? (
            <>
              <div>
                <span className="update-kicker">UPDATE AVAILABLE</span>
                <strong>Env Manager {update?.version}</strong>
                {update?.notes && <p>{update.notes}</p>}
              </div>
              <button type="button" onClick={() => void install()} disabled={state === "installing"}>
                {state === "installing" ? "설치 중…" : "지금 업데이트"}
              </button>
            </>
          ) : state === "current" ? (
            <>
              <strong>최신 버전입니다.</strong>
              <button type="button" className="update-close" onClick={() => setState("idle")}>닫기</button>
            </>
          ) : (
            <>
              <div>
                <strong>업데이트 확인 실패</strong>
                <p>{error}</p>
              </div>
              <button type="button" onClick={() => void runCheck(true)}>다시 확인</button>
            </>
          )}
        </aside>
      )}
    </>
  );
}
