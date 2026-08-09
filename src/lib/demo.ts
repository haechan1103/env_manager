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
    installedVersion: "0.4.0",
    currentVersion: "0.4.0",
    updateAvailable: false,
    protection: "broker",
    detail: "The redacted broker is connected. Direct-file blocking depends on the Codex permission profile.",
    canInstall: true,
  },
  {
    id: "claude-code",
    name: "Claude Code",
    detected: true,
    installed: false,
    installedVersion: null,
    currentVersion: "0.4.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "The tool was detected and can be connected to Env Manager.",
    canInstall: true,
  },
  {
    id: "github-copilot",
    name: "GitHub Copilot / VS Code",
    detected: true,
    installed: false,
    installedVersion: null,
    currentVersion: "0.4.0",
    updateAvailable: false,
    protection: "inactive",
    detail: "VS Code was detected, but the Copilot CLI is required before connecting it.",
    canInstall: false,
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
  gitSafety: {
    state: "protected",
    ignoredFiles: [".env.local", ".env.development", "apps/web/.env.local"],
    missingIgnoreFiles: [],
    trackedFiles: [],
  },
  files: [
    {
      path: ".env.local",
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
            },
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
            },
          ],
        },
      ],
    },
    {
      path: ".env.development",
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
            },
          ],
        },
      ],
    },
    {
      path: "apps/web/.env.local",
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
            },
          ],
        },
      ],
    },
  ],
};
