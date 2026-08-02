import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { OccurrenceProjection } from "../../lib/types";
import { VariableRow } from "./VariableRow";

vi.mock("../../lib/api", () => ({
  saveValue: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: ["GPT_API_KEY"] })),
  saveDescription: vi.fn(async () => ({ affectedFiles: [".env.local"], keys: ["GPT_API_KEY"] })),
  setCodexAccess: vi.fn(async () => undefined),
  readValue: vi.fn(async () => "fake_preview_value"),
  copyValue: vi.fn(async () => undefined),
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
    await user.click(screen.getByTitle("12초 동안 값 보기"));
    expect(await screen.findByDisplayValue("fake_preview_value")).toBeInTheDocument();
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
