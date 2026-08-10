export type CodexAccess = "read-write" | "protected" | "unclassified";
export type ValueState = "empty" | "present";
export type AgentIntegrationId = "codex" | "claude-code" | "github-copilot";
export type AgentProtection = "broker" | "guarded" | "inactive";
export type GitSafetyState = "protected" | "needs-attention" | "not-repository" | "unavailable";

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
  currentVersion: string;
  updateAvailable: boolean;
  protection: AgentProtection;
  detail: string;
  canInstall: boolean;
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
  groups: GroupProjection[];
  warnings: string[];
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
