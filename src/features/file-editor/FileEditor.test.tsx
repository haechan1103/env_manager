import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { ProjectProjection } from "../../lib/types";
import { FileEditor } from "./FileEditor";

vi.mock("../../lib/api", () => ({
  createGroup: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: [] })),
}));

const projection: ProjectProjection = {
  projectId: "demo",
  name: "demo",
  unclassifiedCount: 0,
  issueCount: 0,
  files: [
    {
      path: ".env.local",
      warnings: [],
      groups: [{ name: "GPT", variables: [] }],
    },
  ],
};

describe("FileEditor", () => {
  it("creates an explicit empty group from the file header", async () => {
    const user = userEvent.setup();
    const refresh = vi.fn(async () => undefined);
    render(
      <FileEditor
        projectId="demo"
        projection={projection}
        filePath=".env.local"
        onRefresh={refresh}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    expect(screen.getByText("No variables yet. Add a new variable to this group.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "+ New group" }));
    await user.type(screen.getByLabelText("Group name"), "Database");
    await user.click(screen.getByRole("button", { name: "Create group" }));

    expect(api.createGroup).toHaveBeenCalledWith("demo", {
      file: ".env.local",
      name: "Database",
    });
    expect(refresh).toHaveBeenCalled();
  });
});
