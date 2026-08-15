import { describe, expect, it } from "vitest";

import type { ProjectSummary } from "../lib/types";
import { resolveSelectedProjectId } from "./useEnvManager";

const projects: ProjectSummary[] = [
  { id: "first", name: "First", displayPath: "/fake/first" },
  { id: "surgery", name: "Surgery", displayPath: "/fake/surgery" },
];

describe("resolveSelectedProjectId", () => {
  it("restores the remembered project instead of opening the first project", () => {
    expect(resolveSelectedProjectId(projects, null, "surgery")).toBe("surgery");
  });

  it("keeps a valid in-session selection and falls back from a removed project", () => {
    expect(resolveSelectedProjectId(projects, "surgery", "first")).toBe("surgery");
    expect(resolveSelectedProjectId(projects, null, "removed")).toBe("first");
    expect(resolveSelectedProjectId([], null, "surgery")).toBeNull();
  });
});
