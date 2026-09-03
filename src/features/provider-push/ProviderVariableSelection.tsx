import { useI18n } from "../../i18n";
import type {
  DeploymentProviderId,
  ProjectProjection,
  ProviderEntryKind,
} from "../../lib/types";

type Variable = ProjectProjection["files"][number]["groups"][number]["variables"][number];

export type ProviderSelection = Record<string, {
  selected: boolean;
  kind: ProviderEntryKind;
}>;

interface Props {
  provider: DeploymentProviderId;
  variables: Variable[];
  selection: ProviderSelection;
  ready: boolean;
  isAwsProvider: boolean;
  isRemoteRuntime: boolean;
  onChange: (selection: ProviderSelection) => void;
}

function defaultKind(provider: DeploymentProviderId): ProviderEntryKind {
  return provider === "expo-eas" ? "sensitive" : "secret";
}

export function ProviderVariableSelection({
  provider,
  variables,
  selection,
  ready,
  isAwsProvider,
  isRemoteRuntime,
  onChange,
}: Props) {
  const { t } = useI18n();

  const toggleAll = () => {
    const allSelected = variables
      .filter((item) => item.valueState === "present")
      .every((item) => selection[item.key]?.selected);
    onChange(Object.fromEntries(variables.map((item) => [item.key, {
      selected: item.valueState === "present" && !allSelected,
      kind: selection[item.key]?.kind ?? defaultKind(provider),
    }])));
  };

  return (
    <section className="push-variable-section">
      <header>
        <div><strong>{t("push.selectVariables")}</strong><small>{t("push.valuesHidden")}</small></div>
        <button className="text-button" onClick={toggleAll}>{t("push.selectAll")}</button>
      </header>
      <div className="push-variable-list">
        {!ready ? (
          <div className="push-variable-loading" role="status">
            <span className="spinner" />{t("push.preparing")}
          </div>
        ) : variables.map((variable) => (
          <label
            className={variable.valueState === "empty" ? "push-variable disabled" : "push-variable"}
            key={variable.key}
          >
            <input
              type="checkbox"
              disabled={variable.valueState === "empty"}
              checked={selection[variable.key]?.selected ?? false}
              onChange={(event) => onChange({
                ...selection,
                [variable.key]: {
                  selected: event.target.checked,
                  kind: selection[variable.key]?.kind ?? defaultKind(provider),
                },
              })}
            />
            <span className="push-variable-name">
              <code>{variable.key}</code>
              <small>{variable.valueState === "empty" ? t("push.empty") : t("push.valuePresent")}</small>
            </span>
            {provider === "github-actions" ? (
              <select
                aria-label={t("push.kindFor", { key: variable.key })}
                value={selection[variable.key]?.kind ?? "secret"}
                onChange={(event) => onChange({
                  ...selection,
                  [variable.key]: {
                    selected: selection[variable.key]?.selected ?? false,
                    kind: event.target.value as ProviderEntryKind,
                  },
                })}
              >
                <option value="secret">Secret</option>
                <option value="variable">Variable</option>
              </select>
            ) : provider === "expo-eas" ? (
              <select
                aria-label={t("push.kindFor", { key: variable.key })}
                value={selection[variable.key]?.kind ?? "sensitive"}
                onChange={(event) => onChange({
                  ...selection,
                  [variable.key]: {
                    selected: selection[variable.key]?.selected ?? false,
                    kind: event.target.value as ProviderEntryKind,
                  },
                })}
              >
                <option value="sensitive">Sensitive</option>
                <option value="plaintext">Plain text</option>
                {!variable.key.startsWith("EXPO_PUBLIC_") && <option value="secret">Secret</option>}
              </select>
            ) : (
              <span className="secret-only-badge">
                {provider === "cloudflare-workers"
                  ? t("push.workerSecret")
                  : isAwsProvider
                    ? t("push.awsSecretType")
                    : isRemoteRuntime
                      ? t("runtimeTarget.encryptedCompare")
                      : t("push.stdinSecret")}
              </span>
            )}
          </label>
        ))}
      </div>
    </section>
  );
}
