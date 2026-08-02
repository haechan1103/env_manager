import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { demoProjection } from "../../lib/demo";
import { Overview } from "./Overview";

describe("Overview", () => {
  it("summarizes redacted project state", () => {
    render(
      <Overview
        projection={demoProjection}
        onOpenFile={vi.fn()}
      />,
    );

    expect(screen.getByText("Action inbox")).toBeInTheDocument();
    expect(screen.getByText("NEXT_PUBLIC_APP_URL")).toBeInTheDocument();
    expect(screen.getByText("Codex 접근 미분류")).toBeInTheDocument();
    expect(screen.queryByText("fake_preview_value")).not.toBeInTheDocument();
  });
});
