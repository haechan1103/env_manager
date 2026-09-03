import { useI18n } from "../../i18n";
import type { AwsAccessContext, CloudflareAccessContext } from "../../lib/types";

type Translate = ReturnType<typeof useI18n>["t"];

export function CloudflareAccessStatus({
  access,
  loading,
  failed,
  t,
}: {
  access: CloudflareAccessContext | null;
  loading: boolean;
  failed: boolean;
  t: Translate;
}) {
  if (loading) {
    return (
      <div className="cloudflare-access-status checking" role="status">
        <span className="spinner" />{t("push.cloudflareCheckingAccess")}
      </div>
    );
  }
  if (failed || access?.authState === "unavailable") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareAccessCheckFailed")}</div>;
  }
  if (!access) return null;
  if (access.authState === "not-authenticated") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareNotAuthenticated")}</div>;
  }
  if (access.accountState === "mismatch") {
    return (
      <div className="cloudflare-access-status error">
        {t("push.cloudflareAccountMismatch", { account: access.accountId ?? "-" })}
      </div>
    );
  }
  if (access.targetState !== "accessible") {
    return <div className="cloudflare-access-status error">{t("push.cloudflareTargetUnavailable")}</div>;
  }
  const account = access.accountName
    ? `${access.accountName}${access.accountId ? ` · ${access.accountId}` : ""}`
    : access.accountId ?? t("push.cloudflareWranglerSelectedAccount");
  return (
    <div className="cloudflare-access-status ready">
      <strong>{t("push.cloudflareAccessReady")}</strong>
      <span>{account}</span>
      {access.accountState === "ambiguous" && (
        <small>{t("push.cloudflareAccountAmbiguous", { count: access.accountCount })}</small>
      )}
      {access.adapter.adapterSource === "local-repair" && <small>{t("push.localRepairAdapter")}</small>}
    </div>
  );
}

export function AwsAccessStatus({
  access,
  loading,
  failed,
  t,
}: {
  access: AwsAccessContext | null;
  loading: boolean;
  failed: boolean;
  t: Translate;
}) {
  if (loading) {
    return (
      <div className="cloudflare-access-status checking" role="status">
        <span className="spinner" />{t("push.awsCheckingAccess")}
      </div>
    );
  }
  if (failed) {
    return <div className="cloudflare-access-status error">{t("push.awsAccessFailed")}</div>;
  }
  if (!access) return null;
  return (
    <div className="cloudflare-access-status ready">
      <strong>{t("push.awsAccessReady")}</strong>
      <span>{access.accountId} · {access.region}</span>
      {!access.kmsAliasesAvailable && <small>{t("push.awsKmsListUnavailable")}</small>}
    </div>
  );
}
