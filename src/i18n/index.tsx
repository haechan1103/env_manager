import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export const supportedLocales = [
  { code: "en", label: "English" },
  { code: "ko", label: "한국어" },
] as const;

export type Locale = (typeof supportedLocales)[number]["code"];
type Values = Record<string, string | number>;

import { en, type TranslationKey } from "./locales/en";
import { ko } from "./locales/ko";

export type { TranslationKey } from "./locales/en";

const resources: Record<Locale, Record<TranslationKey, string>> = { en, ko };
const storageKey = "env-manager.locale";

function interpolate(template: string, values: Values = {}) {
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    key in values ? String(values[key]) : match,
  );
}

export function translate(locale: Locale, key: TranslationKey, values?: Values) {
  return interpolate(resources[locale][key], values);
}

const errorKeys: Record<string, TranslationKey> = {
  FILE_CHANGED_EXTERNALLY: "error.fileChanged",
  PARSE_AMBIGUOUS_DUPLICATE_KEY: "error.duplicateKey",
  PARSE_UNSUPPORTED: "error.parseUnsupported",
  PATH_OUTSIDE_REGISTERED_PROJECT: "error.outsideProject",
  UNSUPPORTED_SYMLINK: "error.symlink",
  UNSUPPORTED_ENCODING: "error.encoding",
  FILE_TOO_LARGE: "error.fileTooLarge",
  LINK_VALUE_CONFLICT: "error.linkConflict",
  LINK_MEMBER_MISSING: "error.linkMissing",
  CODEX_ACCESS_BLOCKED: "error.accessBlocked",
  PROTECTION_DOWNGRADE_REQUIRES_CONFIRMATION: "error.confirmationRequired",
  PLAN_EXPIRED: "error.planExpired",
  MULTI_FILE_COMMIT_FAILED: "error.transaction",
  UNREGISTERED_PROJECT: "error.unregistered",
  INVALID_REQUEST: "error.invalidRequest",
  IO_ERROR: "error.io",
  SERIALIZATION_ERROR: "error.serialization",
  AGENT_NOT_FOUND: "error.agentNotFound",
  AGENT_INSTALL_FAILED: "error.agentInstall",
  AGENT_BUNDLE_NOT_UPDATED: "error.agentInstall",
  AGENT_MARKETPLACE_FAILED: "error.agentInstall",
  CLIPBOARD_UNAVAILABLE: "error.clipboard",
  CLIPBOARD_WRITE_FAILED: "error.clipboard",
  PROVIDER_CLI_NOT_FOUND: "error.providerCliMissing",
  PROVIDER_CLI_UNSUPPORTED: "error.providerCliUnsupported",
  PROVIDER_ADAPTER_INVALID: "error.providerCliUnsupported",
  PROVIDER_ADAPTER_STORAGE_UNAVAILABLE: "error.providerCliUnsupported",
  PROVIDER_PUSH_FAILED: "error.providerPush",
  PROVIDER_METADATA_FAILED: "error.providerPush",
  GITHUB_ENVIRONMENT_CREATE_FAILED: "error.providerPush",
  PROVIDER_PAYLOAD_FAILED: "error.providerPush",
  PROVIDER_SELECTION_FAILED: "error.providerPush",
  CLOUDFLARE_NOT_AUTHENTICATED: "error.providerPush",
  CLOUDFLARE_AUTH_CHECK_FAILED: "error.providerPush",
  CLOUDFLARE_ACCOUNT_MISMATCH: "error.providerPush",
  CLOUDFLARE_TARGET_UNAVAILABLE: "error.providerPush",
  AWS_AUTH_UNAVAILABLE: "error.awsAuth",
  AWS_REGION_MISSING: "error.awsRegion",
  AWS_KMS_KEY_UNAVAILABLE: "error.awsKms",
  AWS_KMS_KEY_UNSUPPORTED: "error.awsKms",
  PERSONAL_PROVIDER_INVALID: "error.personalProvider",
  PERSONAL_PROVIDER_UNSAFE_EXECUTABLE: "error.personalProvider",
  PERSONAL_PROVIDER_STORAGE_FAILED: "error.personalProvider",
  ACTION_PACK_INVALID: "error.actionPack",
  ACTION_PACK_EXISTS: "error.actionPack",
  ACTION_PACK_NOT_FOUND: "error.actionPack",
  ACTION_PACK_STORAGE_FAILED: "error.actionPack",
  CREDENTIAL_INVALID_INPUT: "error.credentials",
  CREDENTIAL_NOT_FOUND: "error.credentials",
  CREDENTIAL_PROJECT_NOT_ALLOWED: "error.credentials",
  CREDENTIAL_SECRET_MISSING: "error.credentials",
  CREDENTIAL_STORE_UNAVAILABLE: "error.credentials",
  CREDENTIAL_STORE_FAILED: "error.credentials",
  CREDENTIAL_METADATA_FAILED: "error.credentials",
  ACTION_REQUEST_INVALID: "error.actionPack",
  ACTION_VALUE_SELECTION_FAILED: "error.actionPack",
  ACTION_VALUE_UNREPRESENTABLE: "error.actionPack",
  ACTION_CLI_NOT_FOUND: "error.actionPack",
  ACTION_CLI_UNSUPPORTED: "error.actionPack",
  ACTION_CLI_FAILED: "error.actionPack",
  ACTION_HTTP_FAILED: "error.actionPack",
  PACKAGE_DECRYPT_FAILED: "error.packageDecrypt",
  PACKAGE_INVALID: "error.packageInvalid",
  PACKAGE_CONFLICT: "error.packageConflict",
};

interface LocalizableError {
  code?: unknown;
  message?: unknown;
}

export function localizeError(
  error: unknown,
  locale: Locale,
  fallback: TranslationKey,
  values?: Values,
) {
  const candidate = error as LocalizableError | null;
  const message =
    error instanceof Error
      ? error.message
      : typeof candidate?.message === "string"
        ? candidate.message
        : typeof error === "string"
          ? error
          : null;
  const code = typeof candidate?.code === "string" ? candidate.code : null;

  if (locale === "ko" && message) return message;
  if (code && errorKeys[code]) return translate(locale, errorKeys[code], values);
  if (message && !/[가-힣]/.test(message)) return message;
  return translate(locale, fallback, values);
}

interface I18nValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, values?: Values) => string;
}

const defaultValue: I18nValue = {
  locale: "en",
  setLocale: () => undefined,
  t: (key, values) => translate("en", key, values),
};

const I18nContext = createContext<I18nValue>(defaultValue);

function initialLocale(): Locale {
  try {
    const stored = window.localStorage.getItem(storageKey);
    return stored === "ko" || stored === "en" ? stored : "en";
  } catch {
    return "en";
  }
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<Locale>(initialLocale);
  const t = useCallback(
    (key: TranslationKey, values?: Values) => translate(locale, key, values),
    [locale],
  );

  useEffect(() => {
    document.documentElement.lang = locale;
    try {
      window.localStorage.setItem(storageKey, locale);
    } catch {
      // The app still works when storage is unavailable.
    }
  }, [locale]);

  const value = useMemo(() => ({ locale, setLocale, t }), [locale, t]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}

export function displayGroupName(name: string, t: I18nValue["t"]) {
  return name === "기타" ? t("group.ungrouped") : name;
}
