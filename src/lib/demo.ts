import type {
  AgentIntegrationStatus,
  ProjectProjection,
  ProjectSummary,
} from "./types";

export const demoAgentIntegrations: AgentIntegrationStatus[] = [
  {
    id: "codex",
    name: "Codex",
    detected: true,
    installed: true,
    installedVersion: "1.0.0",
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "broker",
    detail: "The redacted broker is connected. Direct-file blocking depends on the Codex permission profile.",
    canInstall: true,
    actionBlocker: null,
  },
  {
    id: "claude-code",
    name: "Claude Code",
    detected: true,
    installed: false,
    installedVersion: null,
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "The tool was detected and can be connected to Env Manager.",
    canInstall: true,
    actionBlocker: null,
  },
  {
    id: "github-copilot",
    name: "GitHub Copilot / VS Code",
    detected: true,
    installed: false,
    installedVersion: null,
    legacyVersion: false,
    currentVersion: "1.0.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "VS Code was detected, but the Copilot CLI is required before connecting it.",
    canInstall: false,
    actionBlocker: "tool-not-found",
  },
];

export const demoProjects: ProjectSummary[] = [
  {
    id: "demo-project",
    name: "sample-saas",
    displayPath: "/Users/demo/dev/sample-saas",
  },
];

export const demoProjection: ProjectProjection = {
  projectId: "demo-project",
  name: "sample-saas",
  unclassifiedCount: 1,
  issueCount: 0,
  clientExposureCount: 0,
  classificationReview: [
    {
      key: "GPT_API_KEY",
      files: [".env.local", ".env.development"],
      access: "protected",
      classifiedBy: "heuristic",
      suggestion: { access: "protected", reason: "Contains a secret-looking name pattern." },
      clientExposed: false,
    },
    {
      key: "GPT_MODEL",
      files: [".env.local"],
      access: "unclassified",
      classifiedBy: "heuristic",
      suggestion: { access: "unclassified", reason: "Cannot classify safely from its name." },
      clientExposed: false,
    },
  ],
  gitSafety: {
    state: "protected",
    ignoredFiles: [".env.local", ".env.development", "apps/web/.env.local"],
    missingIgnoreFiles: [],
    trackedFiles: [],
    historyFiles: [],
    remoteHistoryFiles: [],
  },
  files: [
    {
      path: ".env.local",
      displayName: "Local environment",
      warnings: [],
      groups: [
        {
          name: "GPT",
          variables: [
            {
              key: "GPT_API_KEY",
              description: ["Server-only API key"],
              valueState: "present",
              displayValue: null,
              codexAccess: "protected",
              linkedCount: 2,
              linkId: "demo-gpt-link",
              linkedFiles: [".env.local", ".env.development"],
              duplicate: false,
              clientExposure: null,
            },
            {
              key: "GPT_MODEL",
              description: ["Default response model"],
              valueState: "present",
              displayValue: null,
              codexAccess: "unclassified",
              linkedCount: 0,
              linkId: null,
              linkedFiles: [],
              duplicate: false,
              clientExposure: null,
            },
            demoVariable("OPENAI_BASE_URL", "Custom API endpoint"),
          ],
        },
        {
          name: "Database",
          variables: [
            demoVariable("DATABASE_URL", "Development database connection", "protected"),
            demoVariable("DATABASE_POOL_SIZE", "Maximum connection pool size"),
            demoVariable("DATABASE_SCHEMA", "Default database schema"),
          ],
        },
        {
          name: "Application",
          variables: [
            {
              key: "PORT",
              description: ["Local development server port"],
              valueState: "present",
              displayValue: null,
              codexAccess: "read-write",
              linkedCount: 0,
              linkId: null,
              linkedFiles: [],
              duplicate: false,
              clientExposure: null,
            },
            demoVariable("APP_ENV", "Application environment"),
            demoVariable("LOG_LEVEL", "Runtime logging level"),
            demoVariable("ENABLE_CACHE", "Enable the local cache"),
            demoVariable("SESSION_TTL", "Session lifetime in seconds"),
          ],
        },
      ],
    },
    {
      path: ".env.development",
      displayName: "Development",
      warnings: [],
      groups: [
        {
          name: "GPT",
          variables: [
            {
              key: "GPT_API_KEY",
              description: ["API key shared in development"],
              valueState: "present",
              displayValue: null,
              codexAccess: "protected",
              linkedCount: 2,
              linkId: "demo-gpt-link",
              linkedFiles: [".env.local", ".env.development"],
              duplicate: false,
              clientExposure: null,
            },
          ],
        },
      ],
    },
    {
      path: "apps/web/.env.local",
      displayName: "Web local",
      warnings: [],
      groups: [
        {
          name: "Web",
          variables: [
            {
              key: "NEXT_PUBLIC_APP_URL",
              description: ["Public application URL"],
              valueState: "empty",
              displayValue: null,
              codexAccess: "read-write",
              linkedCount: 0,
              linkId: null,
              linkedFiles: [],
              duplicate: false,
              clientExposure: null,
            },
          ],
        },
      ],
    },
  ],
};

function demoVariable(
  key: string,
  description: string,
  codexAccess: "protected" | "read-write" = "read-write",
) {
  return {
    key,
    description: [description],
    valueState: "present" as const,
    displayValue: null,
    codexAccess,
    linkedCount: 0,
    linkId: null,
    linkedFiles: [],
    duplicate: false,
    clientExposure: null,
  };
}
