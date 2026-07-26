import type { ProjectProjection, ProjectSummary } from "./types";

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
              description: ["서버에서만 사용하는 API 키입니다."],
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
              description: ["기본 응답 모델"],
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
              description: ["로컬 개발 서버 포트"],
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
              description: ["개발 환경에서 공유하는 API 키"],
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
              description: ["브라우저에 노출되는 앱 주소"],
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
