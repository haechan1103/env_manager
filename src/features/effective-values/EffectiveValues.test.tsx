import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { demoProjection } from "../../lib/demo";
import { EffectiveValues } from "./EffectiveValues";

vi.mock("../../lib/api", () => ({
  getEffectiveValue: vi.fn(async () => ({
    key: "GPT_API_KEY",
    winner: ".env.local",
    shadowed: [".env.development", ".env"],
    reason: "Next.js development 우선순위",
    confidence: "confirmed-profile",
  })),
}));

describe("EffectiveValues", () => {
  it("explains the winning occurrence", async () => {
    const user = userEvent.setup();
    render(
      <EffectiveValues
        projectId="demo"
        projection={demoProjection}
        onError={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "적용 순서 확인" }));
    expect((await screen.findAllByText(".env.local")).length).toBeGreaterThan(0);
    expect(screen.getByText("실제 적용 예상")).toBeInTheDocument();
  });
});
