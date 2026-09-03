import { useEffect, useState } from "react";

import * as api from "../../../lib/api";
import type { AwsAccessContext } from "../../../lib/types";

export function useAwsTarget(active: boolean) {
  const [awsProfile, setAwsProfile] = useState("");
  const [awsRegion, setAwsRegion] = useState("");
  const [awsPathPrefix, setAwsPathPrefix] = useState("");
  const [awsKmsKeyId, setAwsKmsKeyId] = useState("");
  const [awsAccess, setAwsAccess] = useState<AwsAccessContext | null>(null);
  const [loadingAwsAccess, setLoadingAwsAccess] = useState(false);
  const [awsAccessError, setAwsAccessError] = useState(false);

  useEffect(() => {
    if (!active) {
      setAwsAccess(null);
      setLoadingAwsAccess(false);
      setAwsAccessError(false);
      return;
    }
    let current = true;
    setLoadingAwsAccess(true);
    setAwsAccessError(false);
    const timeout = window.setTimeout(() => {
      void api.inspectAwsAccess(awsProfile.trim() || null, awsRegion.trim() || null)
        .then((result) => {
          if (current) setAwsAccess(result);
        })
        .catch(() => {
          if (!current) return;
          setAwsAccess(null);
          setAwsAccessError(true);
        })
        .finally(() => {
          if (current) setLoadingAwsAccess(false);
        });
    }, 350);
    return () => {
      current = false;
      window.clearTimeout(timeout);
    };
  }, [active, awsProfile, awsRegion]);

  return {
    awsProfile,
    setAwsProfile,
    awsRegion,
    setAwsRegion,
    awsPathPrefix,
    setAwsPathPrefix,
    awsKmsKeyId,
    setAwsKmsKeyId,
    awsAccess,
    loadingAwsAccess,
    awsAccessError,
  };
}
