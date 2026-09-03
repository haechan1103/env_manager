import { useI18n, type TranslationKey } from "../../i18n";
import type {
  DeploymentProviderId,
  ProviderCompareResult,
  ProviderPushReceipt,
} from "../../lib/types";
import type { ProviderSelection } from "./ProviderVariableSelection";

interface Props {
  provider: DeploymentProviderId;
  selection: ProviderSelection;
  isRemoteRuntime: boolean;
  latestReceipt: ProviderPushReceipt | null;
  comparison: ProviderCompareResult | null;
}

function comparisonStateKey(state: ProviderCompareResult["items"][number]["state"]): TranslationKey {
  switch (state) {
    case "same": return "compare.state.same";
    case "different": return "compare.state.different";
    case "unset": return "compare.state.unset";
    case "unverifiable": return "compare.state.unverifiable";
    case "error": return "compare.state.error";
  }
}

export function ProviderPushResults({
  provider,
  selection,
  isRemoteRuntime,
  latestReceipt,
  comparison,
}: Props) {
  const { locale, t } = useI18n();

  return (
    <>
      {latestReceipt && (
        <section className="provider-push-receipt">
          <div>
            <strong>{t("pushReceipt.title")}</strong>
            <span>{latestReceipt.destination}</span>
          </div>
          <div>
            <strong>{t("pushReceipt.succeeded", { count: latestReceipt.succeededKeys.length })}</strong>
            <span>{new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(latestReceipt.timestampMs)}</span>
          </div>
          <p>{t("pushReceipt.notEquality")}</p>
        </section>
      )}

      <div className="provider-push-warning">
        <strong>{t(isRemoteRuntime ? "runtimeTarget.networkTitle" : "push.networkTitle")}</strong>
        <p>{t(isRemoteRuntime ? "runtimeTarget.networkBody" : "push.networkBody")}</p>
        {provider === "github-actions"
          && Object.values(selection).some((item) => item.kind === "variable") && (
          <p className="provider-variable-warning">{t("push.variableVisible")}</p>
        )}
        {provider === "expo-eas" && (
          <p className="provider-variable-warning">{t("push.easVisibilityHelp")}</p>
        )}
      </div>

      {comparison && (
        <section className="provider-comparison" aria-live="polite">
          <header>
            <div>
              <strong>{t("compare.resultTitle")}</strong>
              <small>{t("compare.target", { target: comparison.target })}</small>
            </div>
            <span>{t("compare.checkedNow")}</span>
          </header>
          <div className="provider-comparison-list">
            {comparison.items.map((item) => (
              <div className={`provider-comparison-row ${item.state}`} key={item.key}>
                <span><code>{item.key}</code><small>{item.remoteName}</small></span>
                <strong>{t(comparisonStateKey(item.state))}</strong>
              </div>
            ))}
          </div>
          <p>{t("compare.redactedHelp")}</p>
        </section>
      )}
    </>
  );
}
