import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { demoProjection } from "../../lib/demo";
import { Overview } from "./Overview";

describe("Overview", () => {
  it("summarizes redacted project state", () => {
    render(
      <Overview
        projection={demoProjection}
        onOpenFile={vi.fn()}
        onApplyGitignoreGuard={vi.fn()}
      />,
    );

    expect(screen.getByText("Action inbox")).toBeInTheDocument();
    expect(screen.getByText("NEXT_PUBLIC_APP_URL")).toBeInTheDocument();
    expect(screen.getByText("Unclassified AI access")).toBeInTheDocument();
    expect(screen.getByText("All managed env files are ignored")).toBeInTheDocument();
    expect(screen.getByText("2 values blocked")).toBeInTheDocument();
    expect(screen.queryByText("fake_preview_value")).not.toBeInTheDocument();
  });

  it("offers an explicit fix for env files missing Git ignore coverage", async () => {
    const user = userEvent.setup();
    const apply = vi.fn().mockResolvedValue(undefined);
    render(
      <Overview
        projection={{
          ...demoProjection,
          gitSafety: {
            state: "needs-attention",
            ignoredFiles: [".env.local"],
            missingIgnoreFiles: ["apps/web/.env.local"],
            trackedFiles: ["apps/api/.env.development"],
          },
        }}
        onOpenFile={vi.fn()}
        onApplyGitignoreGuard={apply}
      />,
    );

    expect(screen.getByText("Git leak risk")).toBeInTheDocument();
    expect(screen.getAllByText("apps/web/.env.local")).toHaveLength(2);
    expect(screen.getByText("apps/api/.env.development")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add exact .gitignore rules" }));
    expect(apply).toHaveBeenCalledOnce();
  });
});
