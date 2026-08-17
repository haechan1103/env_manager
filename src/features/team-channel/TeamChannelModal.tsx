import { useCallback, useEffect, useState } from "react";

import { Modal } from "../../components/Modal";
import { localizeError, useI18n } from "../../i18n";
import * as api from "../../lib/api";
import type { TeamChannel } from "../../lib/types";

interface Props {
  projectId: string;
  onClose: () => void;
  onError: (message: string) => void;
  onNotice: (message: string) => void;
  onPublish: (channelId: string) => void;
  onImport: (channelId: string, packageId: string) => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.ceil(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function TeamChannelModal({
  projectId,
  onClose,
  onError,
  onNotice,
  onPublish,
  onImport,
}: Props) {
  const { locale, t } = useI18n();
  const [channels, setChannels] = useState<TeamChannel[]>([]);
  const [loading, setLoading] = useState(true);
  const [connecting, setConnecting] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setChannels(await api.listTeamChannels(projectId));
    } catch (error) {
      onError(localizeError(error, locale, "teamChannel.loadError"));
    } finally {
      setLoading(false);
    }
  }, [locale, onError, projectId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const connect = async () => {
    setConnecting(true);
    try {
      const channel = await api.connectFolderTeamChannel(projectId, locale);
      if (channel) {
        onNotice(t("teamChannel.connected", { name: channel.name }));
        await refresh();
      }
    } catch (error) {
      onError(localizeError(error, locale, "teamChannel.connectError"));
    } finally {
      setConnecting(false);
    }
  };

  const remove = async (channel: TeamChannel) => {
    if (!window.confirm(t("teamChannel.removeConfirm", { name: channel.name }))) return;
    try {
      await api.removeTeamChannel(projectId, channel.id);
      setChannels((current) => current.filter((item) => item.id !== channel.id));
      onNotice(t("teamChannel.removed"));
    } catch (error) {
      onError(localizeError(error, locale, "teamChannel.removeError"));
    }
  };

  return (
    <Modal
      className="team-channel-modal"
      title={t("teamChannel.title")}
      description={t("teamChannel.description")}
      onClose={onClose}
    >
      <div className="team-channel-toolbar">
        <p>{t("teamChannel.storageBoundary")}</p>
        <button className="primary-button" disabled={connecting} onClick={() => void connect()}>
          {connecting ? t("teamChannel.connecting") : t("teamChannel.connect")}
        </button>
      </div>

      {loading ? (
        <div className="team-channel-loading" aria-live="polite">
          <span className="spinner" />
          <span>{t("teamChannel.loading")}</span>
        </div>
      ) : channels.length === 0 ? (
        <div className="team-channel-empty">
          <strong>{t("teamChannel.empty")}</strong>
          <p>{t("teamChannel.emptyBody")}</p>
        </div>
      ) : (
        <div className="team-channel-list">
          {channels.map((channel) => (
            <section className="team-channel-card" key={channel.id}>
              <header>
                <div>
                  <strong>{channel.name}</strong>
                  <span className={channel.publishable ? "channel-capability ready" : channel.readable ? "channel-capability readonly" : "channel-capability unavailable"}>
                    {t(channel.publishable ? "teamChannel.readWrite" : channel.readable ? "teamChannel.readOnly" : "teamChannel.unavailable")}
                  </span>
                </div>
                <div className="team-channel-actions">
                  <button className="quiet-button compact" disabled={!channel.publishable} onClick={() => onPublish(channel.id)}>
                    {t("teamChannel.publish")}
                  </button>
                  <button className="danger-quiet-button compact" onClick={() => void remove(channel)}>
                    {t("common.delete")}
                  </button>
                </div>
              </header>
              {!channel.readable ? (
                <p className="team-channel-access-help">{t("teamChannel.accessHelp")}</p>
              ) : channel.packages.length === 0 ? (
                <p className="team-channel-package-empty">{t("teamChannel.noPackages")}</p>
              ) : (
                <div className="team-channel-packages">
                  {channel.packages.map((teamPackage) => (
                    <div key={teamPackage.id}>
                      <span>
                        <code>{teamPackage.id}</code>
                        <small>
                          {teamPackage.modifiedAtMs > 0
                            ? new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(teamPackage.modifiedAtMs)
                            : t("teamChannel.unknownTime")}
                          {` · ${formatBytes(teamPackage.byteSize)}`}
                        </small>
                      </span>
                      <button className="quiet-button compact" onClick={() => onImport(channel.id, teamPackage.id)}>
                        {t("teamChannel.review")}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </section>
          ))}
        </div>
      )}

      <div className="modal-actions">
        <button className="quiet-button" disabled={loading} onClick={() => void refresh()}>{t("common.refresh")}</button>
        <button className="primary-button" onClick={onClose}>{t("common.close")}</button>
      </div>
    </Modal>
  );
}
