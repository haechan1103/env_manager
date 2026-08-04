import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as updateApi from "./updateApi";
import { AppUpdater } from "./AppUpdater";

vi.mock("./updateApi", () => ({
  currentAppVersion: vi.fn(async () => "0.3.0"),
  checkForAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(async () => undefined),
}));

describe("AppUpdater", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(updateApi.checkForAppUpdate).mockResolvedValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("checks quietly after launch and offers a signed update", async () => {
    vi.mocked(updateApi.checkForAppUpdate).mockResolvedValue({
      currentVersion: "0.3.0",
      version: "0.3.1",
      notes: "작고 중요한 안정성 개선",
    });
    render(<AppUpdater />);

    await act(async () => vi.advanceTimersByTimeAsync(1500));

    expect(screen.getByText("Env Manager 0.3.1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "지금 업데이트" }));
    await act(async () => undefined);
    expect(updateApi.installAppUpdate).toHaveBeenCalledOnce();
  });

  it("shows a concise result when the user checks manually", async () => {
    render(<AppUpdater />);
    const versionButton = screen.getByRole("button", { name: /업데이트 확인/ });

    fireEvent.click(versionButton);
    await act(async () => undefined);

    expect(screen.getByText("최신 버전입니다.")).toBeInTheDocument();
  });
});
