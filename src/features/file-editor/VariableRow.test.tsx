import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import * as api from "../../lib/api";
import type { OccurrenceProjection } from "../../lib/types";
import { VariableRow } from "./VariableRow";

vi.mock("../../lib/api", () => ({
  saveValue: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: ["GPT_API_KEY"] })),
  saveDescription: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: ["GPT_API_KEY"] })),
  setCodexAccess: vi.fn(async () => undefined),
  readValue: vi.fn(async () => "fake_preview_value"),
  copyValue: vi.fn(async () => undefined),
  copyKey: vi.fn(async () => undefined),
  detachLink: vi.fn(async () => undefined),
}));

const variable: OccurrenceProjection = {
  key: "GPT_API_KEY",
  description: ["Server-only key"],
  valueState: "present",
  displayValue: null,
  codexAccess: "protected",
  linkedCount: 3,
  linkId: "gpt-link",
  linkedFiles: [".env.local", ".env.development", "apps/api/.env"],
  duplicate: false,
  clientExposure: null,
};

describe("VariableRow", () => {
  it("shows linked save impact after a masked replacement", async () => {
    const user = userEvent.setup();
    render(
      <VariableRow
        projectId="demo"
        file=".env.local"
        variable={variable}
        currentGroup="GPT"
        groups={["GPT", "App"]}
        sameKeyFiles={[".env.local", ".env.development", "apps/api/.env"]}
        onMutate={async (operation) => {
          await operation();
        }}
        onLink={vi.fn()}
      />,
    );

    const input = screen.getByLabelText("GPT_API_KEY value");
    expect(input).toHaveAttribute("placeholder", expect.stringContaining("Value set"));
    await user.type(input, "fake_replacement");
    expect(screen.getByRole("button", { name: "Save to 3 files" })).toBeInTheDocument();
  });

  it("reveals a value only after an explicit action", async () => {
    const user = userEvent.setup();
    render(
      <VariableRow
        projectId="demo"
        file=".env.local"
        variable={variable}
        currentGroup="GPT"
        groups={["GPT", "App"]}
        sameKeyFiles={[".env.local", ".env.development", "apps/api/.env"]}
        onMutate={async (operation) => {
          await operation();
        }}
        onLink={vi.fn()}
      />,
    );

    expect(screen.queryByDisplayValue("fake_preview_value")).not.toBeInTheDocument();
    await user.click(screen.getByTitle("Show value · hides after 30 seconds of inactivity"));
    const revealed = await screen.findByDisplayValue("fake_preview_value");
    expect(revealed).toBeInTheDocument();
    expect(revealed.tagName).toBe("TEXTAREA");
    expect(revealed).toHaveClass("revealed-value-field");
  });

  it("keeps a revealed value visible until 30 seconds of inactivity", async () => {
    vi.useFakeTimers();
    try {
      render(
        <VariableRow
          projectId="demo"
          file=".env.local"
          variable={variable}
          currentGroup="GPT"
          groups={["GPT", "App"]}
          sameKeyFiles={[".env.local", ".env.development", "apps/api/.env"]}
          onMutate={async (operation) => {
            await operation();
          }}
          onLink={vi.fn()}
        />,
      );

      await act(async () => {
        fireEvent.click(screen.getByTitle("Show value · hides after 30 seconds of inactivity"));
      });
      const revealed = screen.getByDisplayValue("fake_preview_value");

      act(() => vi.advanceTimersByTime(20_000));
      fireEvent.keyDown(revealed, { key: "ArrowRight" });
      act(() => vi.advanceTimersByTime(20_000));
      expect(screen.getByDisplayValue("fake_preview_value")).toBeInTheDocument();

      act(() => vi.advanceTimersByTime(10_001));
      expect(screen.queryByDisplayValue("fake_preview_value")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows every file in a linked peer group", () => {
    render(
      <VariableRow
        projectId="demo"
        file=".env.local"
        variable={variable}
        currentGroup="GPT"
        groups={["GPT", "App"]}
        sameKeyFiles={[".env.local", ".env.development", "apps/api/.env"]}
        onMutate={async (operation) => {
          await operation();
        }}
        onLink={vi.fn()}
      />,
    );

    expect(screen.getByText("Managed together in 3 files")).toBeInTheDocument();
    expect(screen.getByText(".env.development")).toBeInTheDocument();
    expect(screen.getByText("apps/api/.env")).toBeInTheDocument();
  });

  it("copies the environment variable name independently from its value", async () => {
    const user = userEvent.setup();
    render(
      <VariableRow
        projectId="demo"
        file=".env.local"
        variable={variable}
        currentGroup="GPT"
        groups={["GPT", "App"]}
        sameKeyFiles={[".env.local", ".env.development", "apps/api/.env"]}
        onMutate={async (operation) => {
          await operation();
        }}
        onLink={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Copy GPT_API_KEY environment variable name" }));

    expect(api.copyKey).toHaveBeenCalledWith("demo", "GPT_API_KEY");
    expect(screen.getByTitle("Copied")).toBeInTheDocument();
  });

  it("suggests an explicit link for unlinked same-name occurrences", () => {
    const unlinked = {
      ...variable,
      linkedCount: 0,
      linkId: null,
      linkedFiles: [],
    };

    render(
      <VariableRow
        projectId="demo"
        file=".env.local"
        variable={unlinked}
        currentGroup="GPT"
        groups={["GPT", "App"]}
        sameKeyFiles={[".env.local", ".env.development"]}
        onMutate={async (operation) => {
          await operation();
        }}
        onLink={vi.fn()}
      />,
    );

    expect(screen.getByText("The same key exists in 2 files")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Manage together" })).toBeInTheDocument();
  });
});
