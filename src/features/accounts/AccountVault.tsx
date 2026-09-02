import { useEffect, useState, type FormEvent } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { AccountProjection, CreateAccountRequest, UpdateAccountRequest } from "../../lib/types";

interface Props {
  projectId: string;
  projectName: string;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
}
export function AccountVault({ projectId, projectName, onError, onNotice }: Props) {
  const { locale, t } = useI18n();
  const [accounts, setAccounts] = useState<AccountProjection[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editor, setEditor] = useState<AccountProjection | "new" | null>(null);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void api.listAccounts(projectId).then((next) => {
      if (active) setAccounts(next);
    }).catch((error: unknown) => {
      if (active) onError(localizeError(error, locale, "error.credentials"));
    }).finally(() => {
      if (active) setLoading(false);
    });
    return () => { active = false; };
  }, [locale, onError, projectId]);

  const setAccess = async (account: AccountProjection, allowed: boolean) => {
    setBusyId(account.id);
    try {
      await api.setAccountProjectAccess(projectId, account.id, allowed);
      setAccounts((current) => current.map((candidate) => candidate.id === account.id ? {
        ...candidate,
        allowedForProject: allowed,
        allowedProjectCount: Math.max(0, candidate.allowedProjectCount + (allowed ? 1 : -1)),
      } : candidate));
      onNotice(t(allowed ? "accounts.allowedNotice" : "accounts.revokedNotice", { name: account.displayName }));
    } catch (error) {
      onError(localizeError(error, locale, "error.credentials"));
    } finally {
      setBusyId(null);
    }
  };

  const copy = async (account: AccountProjection, field: "username" | "password") => {
    setBusyId(account.id);
    try {
      await api.copyAccountField(projectId, account.id, field);
      onNotice(t(field === "username" ? "accounts.usernameCopied" : "accounts.passwordCopied"));
    } catch (error) {
      onError(localizeError(error, locale, "error.credentials"));
    } finally {
      setBusyId(null);
    }
  };

  const remove = async (account: AccountProjection) => {
    if (!window.confirm(t("accounts.deleteConfirm", { name: account.displayName }))) return;
    setBusyId(account.id);
    try {
      await api.deleteAccount(projectId, account.id);
      setAccounts((current) => current.filter((candidate) => candidate.id !== account.id));
      onNotice(t("accounts.deletedNotice", { name: account.displayName }));
    } catch (error) {
      onError(localizeError(error, locale, "error.credentials"));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <section className="account-vault page-stack">
      <header className="account-vault-heading">
        <div>
          <p className="eyebrow">LOCAL · OS PROTECTED</p>
          <h2>{t("accounts.title")}</h2>
          <p>{t("accounts.subtitle")}</p>
        </div>
        <button className="primary-button" onClick={() => setEditor("new")}>{t("accounts.add")}</button>
      </header>

      <aside className="account-security-note">
        <strong>{t("accounts.securityTitle")}</strong>
        <p>{t("accounts.securityBody")}</p>
        <span>{t("accounts.eligibilityNote")}</span>
      </aside>

      {loading ? (
        <div className="account-empty"><span className="spinner" />{t("accounts.loading")}</div>
      ) : accounts.length === 0 ? (
        <div className="account-empty">
          <strong>{t("accounts.empty")}</strong>
          <p>{t("accounts.emptyBody")}</p>
        </div>
      ) : (
        <div className="account-list">
          {accounts.map((account) => {
            const busy = busyId === account.id;
            return (
              <article className={`account-card${account.allowedForProject ? " allowed" : ""}`} key={account.id}>
                <div className="account-card-copy">
                  <div className="account-card-title">
                    <strong>{account.displayName}</strong>
                    <span className={account.allowedForProject ? "account-status allowed" : "account-status"}>
                      {t(account.allowedForProject ? "accounts.allowed" : "accounts.notAllowed")}
                    </span>
                  </div>
                  <p>{account.service}</p>
                  <small>{t("accounts.osStorage")} · {t("accounts.allowedCount", { count: account.allowedProjectCount })}</small>
                </div>
                <div className="account-card-actions">
                  <button className="quiet-button" disabled={!account.allowedForProject || busy} onClick={() => void copy(account, "username")}>{t("accounts.copyUsername")}</button>
                  <button className="quiet-button" disabled={!account.allowedForProject || busy} onClick={() => void copy(account, "password")}>{t("accounts.copyPassword")}</button>
                  <button className="quiet-button" disabled={busy} onClick={() => setEditor(account)}>{t("common.edit")}</button>
                  <button
                    className={account.allowedForProject ? "danger-quiet-button" : "primary-button compact"}
                    disabled={busy}
                    onClick={() => void setAccess(account, !account.allowedForProject)}
                  >
                    {t(account.allowedForProject ? "accounts.revoke" : "accounts.allow", { project: projectName })}
                  </button>
                  <button className="danger-quiet-button" disabled={busy} onClick={() => void remove(account)}>{t("common.remove")}</button>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {editor && (
        <AccountEditorModal
          account={editor === "new" ? null : editor}
          projectId={projectId}
          projectName={projectName}
          onClose={() => setEditor(null)}
          onCreated={(account) => {
            setAccounts((current) => [...current, account].sort((left, right) => left.displayName.localeCompare(right.displayName)));
            setEditor(null);
            onNotice(t("accounts.createdNotice", { name: account.displayName }));
          }}
          onUpdated={(accountId, request) => {
            setAccounts((current) => current.map((account) => account.id === accountId ? {
              ...account,
              displayName: request.displayName,
              service: request.service,
            } : account));
            setEditor(null);
            onNotice(t("accounts.updatedNotice"));
          }}
          onError={onError}
        />
      )}
    </section>
  );
}

interface EditorProps {
  account: AccountProjection | null;
  projectId: string;
  projectName: string;
  onClose: () => void;
  onCreated: (account: AccountProjection) => void;
  onUpdated: (accountId: string, request: UpdateAccountRequest) => void;
  onError: (message: string) => void;
}

function AccountEditorModal({ account, projectId, projectName, onClose, onCreated, onUpdated, onError }: EditorProps) {
  const { locale, t } = useI18n();
  const [displayName, setDisplayName] = useState(account?.displayName ?? "");
  const [service, setService] = useState(account?.service ?? "");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [allowCurrentProject, setAllowCurrentProject] = useState(false);
  const [saving, setSaving] = useState(false);
  const editing = account !== null;
  const ready = Boolean(displayName.trim() && service.trim() && (editing || (username && password)));

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!ready) return;
    setSaving(true);
    try {
      if (account) {
        const request: UpdateAccountRequest = {
          accountId: account.id,
          displayName: displayName.trim(),
          service: service.trim(),
          username: username || null,
          password: password || null,
        };
        await api.updateAccount(projectId, request);
        onUpdated(account.id, request);
      } else {
        const request: CreateAccountRequest = {
          displayName: displayName.trim(),
          service: service.trim(),
          username,
          password,
          allowCurrentProject,
        };
        onCreated(await api.createAccount(projectId, request));
      }
    } catch (error) {
      onError(localizeError(error, locale, "error.credentials"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal title={t(editing ? "accounts.editTitle" : "accounts.createTitle")} description={t("accounts.editorDescription")} onClose={onClose}>
      <form className="modal-form account-editor-form" onSubmit={(event) => void submit(event)}>
        <label><span>{t("accounts.displayName")}</span><input autoFocus value={displayName} maxLength={120} onChange={(event) => setDisplayName(event.target.value)} /></label>
        <label><span>{t("accounts.service")}</span><input value={service} maxLength={240} placeholder={t("accounts.servicePlaceholder")} onChange={(event) => setService(event.target.value)} /></label>
        <label><span>{t("accounts.username")}</span><input value={username} autoComplete="off" placeholder={editing ? t("accounts.keepUsername") : ""} onChange={(event) => setUsername(event.target.value)} /></label>
        <label><span>{t("accounts.password")}</span><input type="password" value={password} autoComplete="new-password" placeholder={editing ? t("accounts.keepPassword") : ""} onChange={(event) => setPassword(event.target.value)} /></label>
        {!editing && (
          <label className="account-grant-choice">
            <input type="checkbox" checked={allowCurrentProject} onChange={(event) => setAllowCurrentProject(event.target.checked)} />
            <span>{t("accounts.allowCurrent", { project: projectName })}</span>
          </label>
        )}
        <p className="account-editor-security">{t("accounts.editorSecurity")}</p>
        <div className="modal-actions">
          <button type="button" className="quiet-button" onClick={onClose}>{t("common.cancel")}</button>
          <button type="submit" className="primary-button" disabled={!ready || saving}>{saving ? t("common.saving") : t("common.save")}</button>
        </div>
      </form>
    </Modal>
  );
}
