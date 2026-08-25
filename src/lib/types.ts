export type CodexAccess = "read-write" | "protected" | "unclassified";
export type ValueState = "empty" | "present";
export type AgentIntegrationId = "codex" | "claude-code" | "github-copilot";
export type AgentProtection = "broker" | "guarded" | "inactive";
export type AgentIntegrationBlocker = "tool-not-found" | "broker-unavailable" | "bundle-unavailable";
export type GitSafetyState = "protected" | "needs-attention" | "not-repository" | "unavailable";
export type DeploymentProviderId = string;
export type ProviderEntryKind = "secret" | "variable" | "plaintext" | "sensitive";
export type AdapterSource = "bundled" | "local-repair" | "personal";
export type DeploymentProviderSource = "official" | "personal";
export type CloudflareAuthState = "authenticated" | "not-authenticated" | "unavailable";
export type CloudflareAccountState = "matched" | "mismatch" | "ambiguous" | "unconfigured" | "unchecked";
export type CloudflareTargetState = "accessible" | "unavailable" | "unchecked";

export interface AdapterStatus {
  cliVersion: string;
  profileId: string;
  adapterVersion: string;
  adapterSource: AdapterSource;
}

export interface DeploymentProviderStatus {
  id: DeploymentProviderId;
  name: string;
  available: boolean;
  detail: string;
  source: DeploymentProviderSource;
  version: string | null;
  targetLabel: string | null;
  adapter: AdapterStatus | null;
}

export interface GitHubRepositoryOptions {
  repositories: string[];
}

export interface GitHubRepositoryContext {
  repository: string | null;
}

export interface GitHubEnvironmentOptions {
  repository: string;
  environments: string[];
}

export interface CloudflareTargetContext {
  worker: string | null;
  environments: string[];
  configPath: string | null;
  accountId: string | null;
  environmentAccountIds: Record<string, string>;
}

export interface EasTargetContext {
  project: string | null;
  projectId: string | null;
  environments: string[];
  configPath: string | null;
}

export interface EasAccessContext {
  project: string;
  projectId: string;
  adapter: AdapterStatus;
}

export interface CloudflareAccessContext {
  authState: CloudflareAuthState;
  authType: string | null;
  accountState: CloudflareAccountState;
  accountId: string | null;
  accountName: string | null;
  accountCount: number;
  targetState: CloudflareTargetState;
  adapter: AdapterStatus;
}

export interface AwsAccessContext {
  accountId: string;
  principalArn: string | null;
  region: string;
  kmsAliases: string[];
  kmsAliasesAvailable: boolean;
}

export interface ProviderPushRequest {
  provider: DeploymentProviderId;
  file: string;
  selections: Array<{ key: string; kind: ProviderEntryKind }>;
  repository: string | null;
  githubEnvironment: string | null;
  worker: string | null;
  cloudflareEnvironment: string | null;
  easProject: string | null;
  easEnvironments: string[];
  personalTarget: string | null;
  awsProfile: string | null;
  awsRegion: string | null;
  awsPathPrefix: string | null;
  awsKmsKeyId: string | null;
}

export interface ProviderPushResult {
  provider: DeploymentProviderId;
  pushedCount: number;
  failedKeys: string[];
}

export type ProviderComparisonState = "same" | "different" | "unset" | "unverifiable" | "error";

export interface ProviderCompareRequest {
  provider: DeploymentProviderId;
  file: string;
  keys: string[];
  awsProfile: string | null;
  awsRegion: string | null;
  awsPathPrefix: string | null;
  runtimeTargetId: string | null;
}

export type RuntimeTransport =
  | { type: "ssh"; destination: string }
  | {
      type: "ecs";
      cluster: string;
      task: string;
      container: string | null;
      profile: string | null;
      region: string | null;
    };

export interface RuntimeTarget {
  id: string;
  displayName: string;
  sourceFile: string;
  remoteTargetId: string;
  recipient: string;
  transport: RuntimeTransport;
}

export interface ProviderComparisonItem {
  key: string;
  remoteName: string;
  state: ProviderComparisonState;
  resultCode: string | null;
}

export interface ProviderCompareResult {
  provider: DeploymentProviderId;
  target: string;
  items: ProviderComparisonItem[];
}

export interface ProviderPushReceipt {
  timestampMs: number;
  projectId: string;
  provider: DeploymentProviderId;
  sourceFile: string;
  destination: string;
  succeededKeys: string[];
  failedKeys: string[];
}

export interface PersonalProviderPackInfo {
  id: string;
  displayName: string;
  description: string;
  version: string;
  targetLabel: string | null;
  available: boolean;
  cliVersion: string | null;
  profileId: string | null;
}

export interface GitSafetyProjection {
  state: GitSafetyState;
  ignoredFiles: string[];
  missingIgnoreFiles: string[];
  trackedFiles: string[];
  historyFiles: string[];
  remoteHistoryFiles: string[];
}

export interface ClientExposureWarning {
  publicPrefix: string;
  secretIndicator: string;
}

export interface GitignoreUpdateSummary {
  addedPatterns: string[];
  trackedFiles: string[];
}

export interface AgentIntegrationStatus {
  id: AgentIntegrationId;
  name: string;
  detected: boolean;
  installed: boolean;
  installedVersion: string | null;
  legacyVersion: boolean;
  currentVersion: string;
  updateAvailable: boolean;
  needsRepair: boolean;
  protection: AgentProtection;
  detail: string;
  canInstall: boolean;
  actionBlocker: AgentIntegrationBlocker | null;
}

export interface ProjectSummary {
  id: string;
  name: string;
  displayPath: string;
}

export interface OccurrenceProjection {
  key: string;
  description: string[];
  valueState: ValueState;
  displayValue: null;
  codexAccess: CodexAccess;
  linkedCount: number;
  linkId: string | null;
  linkedFiles: string[];
  duplicate: boolean;
  clientExposure: ClientExposureWarning | null;
}

export type ClassificationSource = "heuristic" | "user" | "codex";
export type ClassificationReviewReason = "client-exposure-conflict" | "agent-access-request";

export interface ClassificationReviewProjection {
  key: string;
  files: string[];
  access: CodexAccess;
  classifiedBy: ClassificationSource;
  suggestion: { access: CodexAccess; reason: string };
  clientExposed: boolean;
  reviewReasons: ClassificationReviewReason[];
}

export interface GroupProjection {
  name: string;
  variables: OccurrenceProjection[];
}

export interface FileProjection {
  path: string;
  displayName: string;
  groups: GroupProjection[];
  warnings: string[];
}

export interface ExportResult {
  fileCount: number;
  encrypted: boolean;
  cancelled: boolean;
}

export interface ExportOccurrence {
  file: string;
  key: string;
}

export interface TeamChannelPackage {
  id: string;
  byteSize: number;
  modifiedAtMs: number;
}

export interface TeamChannel {
  id: string;
  name: string;
  readable: boolean;
  publishable: boolean;
  packages: TeamChannelPackage[];
}

export interface TeamChannelPublishSummary {
  packageId: string;
  fileCount: number;
}

export type TeamImportOccurrenceState = "new" | "unchanged" | "conflict";

export interface TeamImportPreview {
  files: Array<{
    path: string;
    targetPath: string;
    occurrences: Array<{
      id: string;
      key: string;
      state: TeamImportOccurrenceState;
      linkId: string | null;
    }>;
  }>;
  newCount: number;
  unchangedCount: number;
  conflictCount: number;
}

export type TeamImportValueSide = "local" | "shared";

export interface TeamImportPlanProjection {
  planId: string;
  expiresInSeconds: number;
  preview: TeamImportPreview;
}

export interface TeamImportSummary {
  addedCount: number;
  updatedCount: number;
  unchangedCount: number;
  keptLocalCount: number;
  affectedFiles: string[];
}

export interface ProjectProjection {
  projectId: string;
  name: string;
  files: FileProjection[];
  unclassifiedCount: number;
  issueCount: number;
  gitSafety: GitSafetyProjection;
  classificationReview: ClassificationReviewProjection[];
  accessReviewCount: number;
  clientExposureCount: number;
}

export interface AgentActivityEvent {
  timestampMs: number;
  projectId: string;
  actor: string;
  category: "structure-inspection" | "value-read" | "provider-compare" | "policy-change" | "mutation";
  operation: string;
  relativePaths: string[];
  variableNames: string[];
  policyDecision: string;
  outcome: "allowed" | "blocked" | "failed";
  resultCode: string;
}

export interface MutationSummary {
  affectedFiles: string[];
  keys: string[];
}

export interface MigrationPlanProjection {
  planId: string;
  expiresInSeconds: number;
  preview: {
    file: string;
    summary: string;
    suggestions: Array<{
      currentMarker: string;
      groupName: string;
    }>;
  };
}

export interface CommandError {
  code?: string;
  message?: string;
}
