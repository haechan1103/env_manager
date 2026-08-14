import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ProjectProjection } from "../../lib/types";
import { ProjectSidebar } from "./ProjectSidebar";

const projection: ProjectProjection = {
  projectId: "demo",
  name: "demo",
  unclassifiedCount: 0,
  issueCount: 0,
  clientExposureCount: 0,
  classificationReview: [],
  gitSafety: {
    state: "protected",
    ignoredFiles: [".env.local"],
    missingIgnoreFiles: [],
    trackedFiles: [],
    historyFiles: [],
    remoteHistoryFiles: [],
  },
  files: [{ path: ".env.local", displayName: ".env.local", warnings: [], groups: [] }],
};

describe("ProjectSidebar", () => {
  it("renames projects from the in-app dialog on double-click", async () => {
    const user = userEvent.setup();
    const renameProject = vi.fn();
    render(
      <ProjectSidebar
        projects={[{ id: "demo", name: "demo", displayPath: "/fake/demo" }]}
        selectedProjectId="demo"
        projection={projection}
        view={{ kind: "overview" }}
        onSelectProject={vi.fn()}
        onSelectView={vi.fn()}
        onRegister={vi.fn()}
        onRenameProject={renameProject}
        onRenameFile={vi.fn()}
      />,
    );

    await user.dblClick(screen.getByRole("button", { name: /demo/ }));
    const input = screen.getByLabelText("New name");
    await user.clear(input);
    await user.type(input, "Demo workspace");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(renameProject).toHaveBeenCalledWith("demo", "Demo workspace");
  });
});
