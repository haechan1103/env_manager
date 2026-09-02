import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { OccurrenceProjection, ProjectProjection } from "../../lib/types";
import { FileEditor } from "./FileEditor";

vi.mock("../../lib/api", () => ({
  createGroup: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: [] })),
}));

const projection: ProjectProjection = {
  projectId: "demo",
  name: "demo",
  unclassifiedCount: 0,
  accessReviewCount: 0,
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
  files: [
    {
      path: ".env.local",
      displayName: ".env.local",
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
        onRenameFile={vi.fn()}
      />,
    );

    expect(screen.getByText("No variables yet. Add a new variable to this group.")).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Jump to environment variable group" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "+ New group" }));
    await user.type(screen.getByLabelText("Group name"), "Database");
    await user.click(screen.getByRole("button", { name: "Create group" }));

    expect(api.createGroup).toHaveBeenCalledWith("demo", {
      file: ".env.local",
      name: "Database",
    });
    expect(refresh).toHaveBeenCalled();
  });

  it("renames a file through the in-app dialog", async () => {
    const user = userEvent.setup();
    const renameFile = vi.fn();
    render(
      <FileEditor
        projectId="demo"
        projection={projection}
        filePath=".env.local"
        onRefresh={vi.fn(async () => undefined)}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onRenameFile={renameFile}
      />,
    );

    expect(screen.queryByText("ENV FILE")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rename env file" }));
    const input = screen.getByLabelText("New name");
    expect(input).toHaveValue(".env.local");
    await user.clear(input);
    await user.type(input, "Local development");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(renameFile).toHaveBeenCalledWith(".env.local", "Local development");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows sticky group shortcuts only for files with at least ten variables", async () => {
    const user = userEvent.setup();
    const scrollIntoView = vi.fn();
    const scrollBy = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;
    HTMLElement.prototype.scrollBy = scrollBy;
    const clientWidth = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(200);
    const scrollWidth = vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockReturnValue(600);
    const largeProjection: ProjectProjection = {
      ...projection,
      files: [{
        ...projection.files[0]!,
        groups: [
          { name: "GPT", variables: Array.from({ length: 5 }, (_, index) => variable(`GPT_${index}`)) },
          { name: "Database", variables: Array.from({ length: 5 }, (_, index) => variable(`DB_${index}`)) },
        ],
      }],
    };

    render(
      <FileEditor
        projectId="demo"
        projection={largeProjection}
        filePath=".env.local"
        onRefresh={vi.fn(async () => undefined)}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onRenameFile={vi.fn()}
      />,
    );

    const navigation = screen.getByRole("navigation", { name: "Jump to environment variable group" });
    expect(navigation).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Database · 5" }));
    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "start" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Next groups" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Next groups" }));
    expect(scrollBy).toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "All groups" }));
    expect(navigation).toHaveClass("expanded");
    expect(screen.queryByRole("button", { name: "Next groups" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Collapse groups" }));
    expect(navigation).not.toHaveClass("expanded");
    expect(screen.getByRole("button", { name: "Next groups" })).toBeInTheDocument();

    clientWidth.mockRestore();
    scrollWidth.mockRestore();
  });

  it("filters the file view to variables whose values are empty", async () => {
    const user = userEvent.setup();
    const filterProjection: ProjectProjection = {
      ...projection,
      files: [{
        ...projection.files[0]!,
        groups: [
          {
            name: "GPT",
            variables: [variable("GPT_API_KEY", "empty"), variable("GPT_MODEL")],
          },
          {
            name: "Database",
            variables: [variable("DATABASE_URL")],
          },
        ],
      }],
    };

    render(
      <FileEditor
        projectId="demo"
        projection={filterProjection}
        filePath=".env.local"
        onRefresh={vi.fn(async () => undefined)}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onRenameFile={vi.fn()}
      />,
    );

    expect(screen.getByText("GPT_API_KEY")).toBeInTheDocument();
    expect(screen.getByText("GPT_MODEL")).toBeInTheDocument();
    expect(screen.getByText("DATABASE_URL")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Missing values only/ }));

    expect(screen.getByText("GPT_API_KEY")).toBeInTheDocument();
    expect(screen.queryByText("GPT_MODEL")).not.toBeInTheDocument();
    expect(screen.queryByText("DATABASE_URL")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Database" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Showing 1 of 3 variables")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Missing values only/ }));
    expect(screen.getByText("DATABASE_URL")).toBeInTheDocument();
  });

  it("explains when the selected file has no empty variables", async () => {
    const user = userEvent.setup();
    const completeProjection: ProjectProjection = {
      ...projection,
      files: [{
        ...projection.files[0]!,
        groups: [{ name: "GPT", variables: [variable("GPT_API_KEY")] }],
      }],
    };

    render(
      <FileEditor
        projectId="demo"
        projection={completeProjection}
        filePath=".env.local"
        onRefresh={vi.fn(async () => undefined)}
        onError={vi.fn()}
        onNotice={vi.fn()}
        onRenameFile={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: /Missing values only/ }));

    expect(screen.getByText("Every variable has a value")).toBeInTheDocument();
    expect(screen.queryByText("GPT_API_KEY")).not.toBeInTheDocument();
  });
});

function variable(
  key: string,
  valueState: OccurrenceProjection["valueState"] = "present",
): OccurrenceProjection {
  return {
    key,
    description: [],
    valueState,
    displayValue: null,
    codexAccess: "protected",
    linkedCount: 1,
    linkId: null,
    linkedFiles: [],
    duplicate: false,
    clientExposure: null,
  };
}
