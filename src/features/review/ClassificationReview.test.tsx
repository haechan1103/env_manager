import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { demoProjection } from "../../lib/demo";
import { ClassificationReview } from "./ClassificationReview";

vi.mock("../../lib/api", () => ({
  setCodexAccess: vi.fn(async () => undefined),
  protectVariables: vi.fn(async () => undefined),
}));

describe("ClassificationReview", () => {
  it("can protect every ambiguous variable without a batch allow action", async () => {
    const user = userEvent.setup();
    const refresh = vi.fn(async () => undefined);
    render(
      <ClassificationReview
        projectId="demo-project"
        projection={demoProjection}
        onRefresh={refresh}
        onOpenFile={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Protect all 1" }));
    expect(api.protectVariables).toHaveBeenCalledWith("demo-project", ["GPT_MODEL"]);
    expect(refresh).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /allow all/i })).not.toBeInTheDocument();
    expect(screen.getAllByText("The name alone is not enough to decide whether an AI tool may read its value.")).toHaveLength(1);
  });

  it("filters pending variables by env file and scopes protect all to that file", async () => {
    const user = userEvent.setup();
    const projection = {
      ...demoProjection,
      classificationReview: [
        ...demoProjection.classificationReview,
        {
          key: "MOBILE_REGION",
          files: ["apps/web/.env.local"],
          access: "unclassified" as const,
          classifiedBy: "heuristic" as const,
          suggestion: { access: "unclassified" as const, reason: "Ambiguous fixture name." },
          clientExposed: false,
        },
      ],
    };
    render(
      <ClassificationReview
        projectId="demo-project"
        projection={projection}
        onRefresh={vi.fn(async () => undefined)}
        onOpenFile={vi.fn()}
        onError={vi.fn()}
        onNotice={vi.fn()}
      />,
    );

    expect(screen.getByRole("tab", { name: "All env files, 2 needing decision" })).toHaveAttribute("aria-selected", "true");
    await user.click(screen.getByRole("tab", { name: "Web local, 1 needing decision" }));

    expect(screen.getByText("MOBILE_REGION")).toBeInTheDocument();
    expect(screen.queryByText("GPT_MODEL")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Protect all 1" }));
    expect(api.protectVariables).toHaveBeenLastCalledWith("demo-project", ["MOBILE_REGION"]);
  });
});
