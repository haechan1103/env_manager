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
  description: ["서버 전용 키"],
  valueState: "present",
  displayValue: null,
  codexAccess: "protected",
  linkedCount: 3,
  linkId: "gpt-link",
  linkedFiles: [".env.local", ".env.development", "apps/api/.env"],
  duplicate: false,
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

    const input = screen.getByLabelText("GPT_API_KEY 값");
    expect(input).toHaveAttribute("placeholder", expect.stringContaining("값 있음"));
    await user.type(input, "fake_replacement");
    expect(screen.getByRole("button", { name: "3개 파일에 저장" })).toBeInTheDocument();
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
    await user.click(screen.getByTitle("값 보기 · 30초 미활동 시 숨김"));
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
        fireEvent.click(screen.getByTitle("값 보기 · 30초 미활동 시 숨김"));
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

    expect(screen.getByText("3개 파일에서 함께 관리")).toBeInTheDocument();
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

    await user.click(screen.getByRole("button", { name: "GPT_API_KEY 환경변수명 복사" }));

    expect(api.copyKey).toHaveBeenCalledWith("demo", "GPT_API_KEY");
    expect(screen.getByTitle("복사됨")).toBeInTheDocument();
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

    expect(screen.getByText("같은 변수가 2개 파일에 있습니다")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "함께 관리" })).toBeInTheDocument();
  });
});
