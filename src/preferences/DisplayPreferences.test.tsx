import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import {
  DisplayPreferencesProvider,
  useDisplayPreferences,
  type FontSize,
} from "./DisplayPreferences";

function FontSizeProbe() {
  const { fontSize, setFontSize } = useDisplayPreferences();
  return (
    <div>
      <span>{fontSize}</span>
      <button onClick={() => setFontSize("large")}>Use large text</button>
    </div>
  );
}

function renderProbe() {
  return render(
    <DisplayPreferencesProvider>
      <FontSizeProbe />
    </DisplayPreferencesProvider>,
  );
}

describe("DisplayPreferencesProvider", () => {
  it("uses the current UI size as the smallest level and persists changes", async () => {
    const user = userEvent.setup();
    renderProbe();

    expect(screen.getByText("small")).toBeInTheDocument();
    expect(document.documentElement.dataset.fontSize).toBe("small");

    await user.click(screen.getByRole("button", { name: "Use large text" }));

    expect(screen.getByText("large")).toBeInTheDocument();
    expect(window.localStorage.getItem("env-manager.font-size")).toBe("large");
    expect(document.documentElement.dataset.fontSize).toBe("large");
  });

  it("restores a supported saved level and ignores invalid values", () => {
    window.localStorage.setItem("env-manager.font-size", "extra-large" satisfies FontSize);
    const first = renderProbe();
    expect(screen.getByText("extra-large")).toBeInTheDocument();
    first.unmount();

    window.localStorage.setItem("env-manager.font-size", "oversized");
    renderProbe();
    expect(screen.getByText("small")).toBeInTheDocument();
  });
});
