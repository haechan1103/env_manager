export type CodexAccess = "read-write" | "protected" | "unclassified";
export type ValueState = "empty" | "present";
export type AgentIntegrationId = "codex" | "claude-code" | "github-copilot";
export type AgentProtection = "broker" | "guarded" | "inactive";
export type AgentIntegrationBlocker = "tool-not-found" | "broker-unavailable" | "bundle-unavailable";
export type GitSafetyState = "protected" | "needs-attention" | "not-repository" | "unavailable";
export type DeploymentProviderId = "github-actions" | "cloudflare-workers";
export type GitHubEntryKind = "secret" | "variable";

export interface DeploymentProviderStatus {
  id: DeploymentProviderId;
  name: string;
  available: boolean;
  detail: string;
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
}

export interface ProviderPushRequest {
  provider: DeploymentProviderId;
  file: string;
  selections: Array<{ key: string; kind: GitHubEntryKind }>;
  repository: string | null;
  githubEnvironment: string | null;
  worker: string | null;
  cloudflareEnvironment: string | null;
}

export interface ProviderPushResult {
  provider: DeploymentProviderId;
  pushedCount: number;
  failedKeys: string[];
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

export interface ClassificationReviewProjection {
  key: string;
  files: string[];
  access: CodexAccess;
  classifiedBy: ClassificationSource;
  suggestion: { access: CodexAccess; reason: string };
  clientExposed: boolean;
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

export type TeamImportOccurrenceState = "new" | "unchanged" | "conflict";

export interface TeamImportPreview {
  files: Array<{
    path: string;
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
  clientExposureCount: number;
}

export interface AgentActivityEvent {
  timestampMs: number;
  projectId: string;
  actor: string;
  category: "structure-inspection" | "value-read" | "policy-change" | "mutation";
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
