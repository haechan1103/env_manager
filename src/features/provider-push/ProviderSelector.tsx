import { useI18n } from "../../i18n";
import type {
  DeploymentProviderId,
  DeploymentProviderStatus,
  RuntimeTarget,
} from "../../lib/types";

interface Props {
  providers: DeploymentProviderStatus[];
  selected: DeploymentProviderId;
  loading: boolean;
  runtimeTargets: RuntimeTarget[];
  installingPack: boolean;
  onSelect: (provider: DeploymentProviderId) => void;
  onInstallPack: () => void;
}

const fallbackProviders = [
  { id: "github-actions", name: "GitHub Actions" },
  { id: "cloudflare-workers", name: "Cloudflare Workers" },
] as const;

function providerMark(id: DeploymentProviderId) {
  switch (id) {
    case "github-actions": return "GH";
    case "cloudflare-workers": return "CF";
    case "expo-eas": return "EA";
    case "aws-secrets-manager": return "AS";
    case "aws-ssm-parameter-store": return "SS";
    case "remote-runtime": return "RT";
    default: return "P";
  }
}

export function ProviderSelector({
  providers,
  selected,
  loading,
  runtimeTargets,
  installingPack,
  onSelect,
  onInstallPack,
}: Props) {
  const { t } = useI18n();
  const options = providers.length > 0 ? providers : fallbackProviders;

  return (
    <div className="push-provider-options">
      {options.map((item) => {
        const id = item.id as DeploymentProviderId;
        const status = providers.find((candidate) => candidate.id === id);
        return (
          <button
            key={id}
            className={selected === id ? "push-provider selected" : "push-provider"}
            onClick={() => onSelect(id)}
          >
            <span className="push-provider-mark">{providerMark(id)}</span>
            <span>
              <strong>{item.name}</strong>
              <small className={loading ? "provider-status-loading" : undefined}>
                {loading && <span className="spinner" />}
                {loading ? t("push.checkingCli") : id === "remote-runtime"
                  ? (runtimeTargets.length > 0 ? t("runtimeTarget.ready") : t("runtimeTarget.missing"))
                  : status?.available ? (
                    <><span>{t("push.cliReady")}</span>{status.adapter && ` · v${status.adapter.cliVersion}`}</>
                  ) : t("push.cliMissing")}
              </small>
              {status?.source === "personal" && (
                <small>{t("push.personalPack", { version: status.version ?? "" })}</small>
              )}
            </span>
          </button>
        );
      })}
      <button className="push-provider add-pack" disabled={installingPack} onClick={onInstallPack}>
        <span className="push-provider-mark">+</span>
        <span><strong>{t("push.addPack")}</strong><small>{t("push.addPackBody")}</small></span>
      </button>
    </div>
  );
}
