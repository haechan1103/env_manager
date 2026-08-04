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

    expect(screen.getByText("아직 변수가 없습니다. 새 변수를 이 그룹에 추가해보세요.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "+ 새 그룹" }));
    await user.type(screen.getByLabelText("그룹 이름"), "Database");
    await user.click(screen.getByRole("button", { name: "그룹 만들기" }));

    expect(api.createGroup).toHaveBeenCalledWith("demo", {
      file: ".env.local",
      name: "Database",
    });
    expect(refresh).toHaveBeenCalled();
  });
});
