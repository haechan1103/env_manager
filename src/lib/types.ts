export type CodexAccess = "read-write" | "protected" | "unclassified";
export type ValueState = "empty" | "present";

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
