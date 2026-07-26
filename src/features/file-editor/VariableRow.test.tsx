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
});
