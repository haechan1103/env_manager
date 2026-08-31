import { useEffect, useMemo, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type {
  ActionExecutionResult,
  ActionPackInfo,
  ProjectProjection,
} from "../../lib/types";

interface Props {
  projectId: string;
  projection: ProjectProjection;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function ActionPackModal({ projectId, projection, onClose, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [packs, setPacks] = useState<ActionPackInfo[]>([]);
  const [packId, setPackId] = useState("");
  const [file, setFile] = useState(projection.files[0]?.path ?? "");
  const [bindings, setBindings] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ActionExecutionResult | null>(null);

  const load = async (preferredId?: string) => {
    setLoading(true);
    try {
      const next = await api.listActionPacks(projectId);
      setPacks(next);
      setPackId((current) => {
        const preferred = preferredId ?? current;
        return next.some((pack) => pack.id === preferred) ? preferred : (next[0]?.id ?? "");
      });
    } catch (error) {
      onError(localizeError(error, locale, "action.error"));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId]);

  const pack = packs.find((candidate) => candidate.id === packId) ?? null;
  const variables = useMemo(
    () => projection.files
      .find((candidate) => candidate.path === file)
      ?.groups.flatMap((group) => group.variables) ?? [],
    [file, projection.files],
  );

  useEffect(() => {
    if (!pack) {
      setBindings({});
      return;
    }
    setBindings(Object.fromEntries(pack.bindings.map((binding) => {
      const exact = variables.find((variable) => variable.key === binding.id && variable.valueState === "present");
      return [binding.id, exact?.key ?? ""];
    })));
    setResult(null);
  }, [pack, variables]);

  const ready = Boolean(pack?.available)
    && pack!.bindings.every((binding) => {
      const selected = bindings[binding.id];
      return variables.some((variable) => variable.key === selected && variable.valueState === "present");
    });

  const install = async () => {
    setBusy(true);
    try {
      const installed = await api.chooseAndInstallActionPack(t("action.installDialog"));
      if (installed) {
        onNotice(t("action.installed", { name: installed.displayName }));
        await load(installed.id);
      }
    } catch (error) {
      onError(localizeError(error, locale, "action.installError"));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!pack || !window.confirm(t("action.removeConfirm", { name: pack.displayName }))) return;
    setBusy(true);
    try {
      await api.removeActionPack(pack.id);
      onNotice(t("action.removed", { name: pack.displayName }));
      await load();
    } catch (error) {
      onError(localizeError(error, locale, "action.removeError"));
    } finally {
      setBusy(false);
    }
  };

  const run = async () => {
    if (!pack || !ready) return;
    setBusy(true);
    setResult(null);
    try {
      setResult(await api.executeActionPack(projectId, { packId: pack.id, file, bindings }));
    } catch (error) {
      onError(localizeError(error, locale, "action.error"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal className="action-pack-modal" title={t("action.title")} description={t("action.description")} onClose={onClose}>
      <div className="action-pack-toolbar">
        <label>
          <span>{t("action.pack")}</span>
          <select disabled={loading || packs.length === 0} value={packId} onChange={(event) => setPackId(event.target.value)}>
            {packs.map((item) => <option key={item.id} value={item.id}>{item.displayName}</option>)}
          </select>
        </label>
        <button className="quiet-button" disabled={busy} onClick={() => void install()}>{t("action.install")}</button>
      </div>

      {loading ? <div className="action-pack-empty"><span className="spinner" />{t("action.loading")}</div>
        : !pack ? <div className="action-pack-empty"><strong>{t("action.emptyTitle")}</strong><p>{t("action.emptyBody")}</p></div>
          : (
            <>
              <section className="action-pack-summary">
                <div><span>{pack.kind.toUpperCase()} · v{pack.packVersion}</span><strong>{pack.displayName}</strong><p>{pack.description}</p></div>
                <button className="danger-quiet-button" disabled={busy} onClick={() => void remove()}>{t("common.remove")}</button>
                <dl><dt>{t("action.target")}</dt><dd>{pack.target}</dd>{pack.cliVersion && <><dt>{t("action.cliVersion")}</dt><dd>{pack.cliVersion}</dd></>}</dl>
              </section>

              <div className="modal-form action-pack-form">
                <label><span>{t("action.file")}</span><select value={file} onChange={(event) => setFile(event.target.value)}>{projection.files.map((item) => <option key={item.path} value={item.path}>{item.displayName}</option>)}</select></label>
                {pack.bindings.map((binding) => (
                  <label key={binding.id}>
                    <span>{binding.id} <em>→ {binding.destination}</em></span>
                    <select value={bindings[binding.id] ?? ""} onChange={(event) => setBindings((current) => ({ ...current, [binding.id]: event.target.value }))}>
                      <option value="">{t("action.chooseVariable")}</option>
                      {variables.map((variable) => <option key={variable.key} value={variable.key} disabled={variable.valueState !== "present"}>{variable.key}{variable.valueState !== "present" ? ` · ${t("action.emptyValue")}` : ""}</option>)}
                    </select>
                  </label>
                ))}
              </div>

              <aside className="action-pack-trust"><strong>{t("action.trustTitle")}</strong><p>{t("action.trustBody")}</p></aside>
              {result && <section className={`action-pack-result ${result.succeeded ? "success" : "failed"}`} aria-live="polite"><strong>{t(result.succeeded ? "action.resultSuccess" : "action.resultFailed")}</strong><div>{result.statusCode !== null && <span>HTTP {result.statusCode}</span>}{result.exitCode !== null && <span>EXIT {result.exitCode}</span>}{result.durationMs !== null && <span>{result.durationMs} ms</span>}<span>{result.resultCode}</span></div></section>}
            </>
          )}
      <div className="modal-actions"><button className="quiet-button" onClick={onClose}>{t("common.close")}</button><button className="primary-button" disabled={!ready || busy} onClick={() => void run()}>{busy ? t("action.running") : t("action.run")}</button></div>
    </Modal>
  );
}
