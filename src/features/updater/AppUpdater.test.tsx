import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as updateApi from "./updateApi";
import { I18nProvider } from "../../i18n";
import { AppUpdater, localizedUpdateNotes } from "./AppUpdater";

vi.mock("./updateApi", () => ({
  currentAppVersion: vi.fn(async () => "0.4.0"),
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
      currentVersion: "0.4.0",
      version: "0.4.1",
      notes: "Small but important reliability improvements",
    });
    render(<AppUpdater />);

    await act(async () => vi.advanceTimersByTimeAsync(1500));

    expect(screen.getByText("Env Manager 0.4.1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Update now" }));
    await act(async () => undefined);
    expect(updateApi.installAppUpdate).toHaveBeenCalledOnce();
  });

  it("shows a concise result when the user checks manually", async () => {
    render(<AppUpdater />);
    const versionButton = screen.getByRole("button", { name: /check for updates/ });

    fireEvent.click(versionButton);
    await act(async () => undefined);

    expect(screen.getByText("You are on the latest version.")).toBeInTheDocument();
  });

  it("shows only the Korean section of bilingual release notes in Korean", async () => {
    window.localStorage.setItem("env-manager.locale", "ko");
    vi.mocked(updateApi.checkForAppUpdate).mockResolvedValue({
      currentVersion: "0.5.0",
      version: "0.5.1",
      notes: "Env Manager v0.5.1 for macOS.\n\n- English note\n\n---\n\nEnv Manager v0.5.1 macOS 설치 파일입니다.\n\n- 한국어 안내",
    });
    render(<I18nProvider><AppUpdater /></I18nProvider>);

    await act(async () => vi.advanceTimersByTimeAsync(1500));

    expect(screen.getByText("업데이트 가능")).toBeInTheDocument();
    expect(screen.getByText(/한국어 안내/)).toBeInTheDocument();
    expect(screen.queryByText(/English note/)).not.toBeInTheDocument();
  });

  it("does not show English-only release notes in Korean", () => {
    expect(localizedUpdateNotes("English-only release notes", "ko")).toBeNull();
  });
});
