import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import { ImportConflictCard } from "./ImportConflictCard";

vi.mock("../../lib/api", () => ({
  revealTeamImportConflict: vi.fn(async () => "fake_conflict_value"),
}));

afterEach(() => vi.useRealTimers());

describe("ImportConflictCard", () => {
  it("keeps one explicitly revealed side visible until 30 seconds of inactivity", async () => {
    vi.useFakeTimers();
    render(
      <ImportConflictCard
        projectId="demo"
        planId="fake-plan"
        occurrences={[{
          id: "fake-conflict",
          key: "TOKEN",
          sourcePath: ".env.local",
          targetPath: ".env.local",
          linkId: null,
        }]}
        useShared={false}
        onChoice={vi.fn()}
        onError={vi.fn()}
      />,
    );

    expect(screen.queryByText("fake_conflict_value")).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Reveal my local value" }));
    });
    expect(api.revealTeamImportConflict).toHaveBeenCalledWith(
      "demo",
      "fake-plan",
      "fake-conflict",
      "local",
    );
    const value = screen.getByText("fake_conflict_value");

    act(() => vi.advanceTimersByTime(20_000));
    fireEvent.keyDown(value, { key: "ArrowRight" });
    act(() => vi.advanceTimersByTime(20_000));
    expect(screen.getByText("fake_conflict_value")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(10_001));
    expect(screen.queryByText("fake_conflict_value")).not.toBeInTheDocument();
  });
});
