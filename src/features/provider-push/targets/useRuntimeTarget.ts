import { useEffect, useState } from "react";

import { localizeError, useI18n } from "../../../i18n";
import * as api from "../../../lib/api";
import type { RuntimeTarget } from "../../../lib/types";

interface Options {
  projectId: string;
  file: string;
  setFile: (file: string) => void;
  active: boolean;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}

export function useRuntimeTarget({
  projectId,
  file,
  setFile,
  active,
  onError,
  onNotice,
}: Options) {
  const { locale, t } = useI18n();
  const [runtimeTargets, setRuntimeTargets] = useState<RuntimeTarget[]>([]);
  const [runtimeTargetId, setRuntimeTargetId] = useState("");
  const [editingRuntimeTarget, setEditingRuntimeTarget] = useState(false);
  const [runtimeDisplayName, setRuntimeDisplayName] = useState("");
  const [runtimeRemoteId, setRuntimeRemoteId] = useState("");
  const [runtimeDestination, setRuntimeDestination] = useState("");
  const [runtimeRecipient, setRuntimeRecipient] = useState("");
  const [savingRuntimeTarget, setSavingRuntimeTarget] = useState(false);

  useEffect(() => {
    if (!active) return;
    let current = true;
    void api.listRuntimeTargets(projectId)
      .then((targets) => {
        if (!current) return;
        setRuntimeTargets(targets);
        const selected = targets.find((target) => target.id === runtimeTargetId) ?? targets[0];
        if (selected) {
          setRuntimeTargetId(selected.id);
          setFile(selected.sourceFile);
        }
      })
      .catch((error) => onError(localizeError(error, locale, "compare.error")));
    return () => { current = false; };
  }, [active, locale, onError, projectId, setFile]);

  const resetRuntimeTargetDraft = () => {
    setRuntimeDisplayName("");
    setRuntimeRemoteId("");
    setRuntimeDestination("");
    setRuntimeRecipient("");
  };

  const saveRuntimeTarget = async () => {
    const remoteId = runtimeRemoteId.trim();
    if (!remoteId || !runtimeDisplayName.trim() || !runtimeDestination.trim() || !runtimeRecipient.trim() || !file) return;
    setSavingRuntimeTarget(true);
    try {
      const targets = await api.saveRuntimeTarget(projectId, {
        id: remoteId,
        displayName: runtimeDisplayName.trim(),
        sourceFile: file,
        remoteTargetId: remoteId,
        recipient: runtimeRecipient.trim(),
        transport: { type: "ssh", destination: runtimeDestination.trim() },
      });
      setRuntimeTargets(targets);
      setRuntimeTargetId(remoteId);
      setEditingRuntimeTarget(false);
      resetRuntimeTargetDraft();
      onNotice(t("runtimeTarget.saved"));
    } catch (error) {
      onError(localizeError(error, locale, "runtimeTarget.saveError"));
    } finally {
      setSavingRuntimeTarget(false);
    }
  };

  const removeRuntimeTarget = async () => {
    if (!runtimeTargetId) return;
    setSavingRuntimeTarget(true);
    try {
      const targets = await api.removeRuntimeTarget(projectId, runtimeTargetId);
      setRuntimeTargets(targets);
      const next = targets[0];
      setRuntimeTargetId(next?.id ?? "");
      if (next) setFile(next.sourceFile);
      onNotice(t("runtimeTarget.removed"));
    } catch (error) {
      onError(localizeError(error, locale, "runtimeTarget.removeError"));
    } finally {
      setSavingRuntimeTarget(false);
    }
  };

  return {
    runtimeTargets,
    runtimeTargetId,
    setRuntimeTargetId,
    editingRuntimeTarget,
    setEditingRuntimeTarget,
    runtimeDisplayName,
    setRuntimeDisplayName,
    runtimeRemoteId,
    setRuntimeRemoteId,
    runtimeDestination,
    setRuntimeDestination,
    runtimeRecipient,
    setRuntimeRecipient,
    savingRuntimeTarget,
    resetRuntimeTargetDraft,
    saveRuntimeTarget,
    removeRuntimeTarget,
  };
}
