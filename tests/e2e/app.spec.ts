import { expect, test } from "@playwright/test";

test("navigates the redacted V1 workflow", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Action inbox")).toBeVisible();
  await expect(page.getByText("NEXT_PUBLIC_APP_URL")).toBeVisible();
  await expect(page.getByText("fake_preview_value")).toHaveCount(0);

  await page.getByRole("button", { name: ".env.local", exact: true }).click();
  await expect(page.getByRole("heading", { name: ".env.local" })).toBeVisible();
  await page.getByRole("button", { name: "기존 주석 정리" }).click();
  await expect(page.getByRole("heading", { name: "기존 env 주석 정리 계획" })).toBeVisible();
  await expect(page.getByText("# @group GPT")).toBeVisible();
  await expect(page.getByText("값은 계획과 화면에 포함되지 않습니다.")).toBeVisible();
  await page.getByRole("button", { name: "취소" }).click();
  const apiKeyInput = page.getByLabel("GPT_API_KEY 값");
  await apiKeyInput.fill("fake_e2e_replacement");
  await expect(page.getByRole("button", { name: "2개 파일에 저장" })).toBeVisible();
  await page.screenshot({
    path: "test-results/env-manager-file-editor.png",
    fullPage: true,
  });

  await page.getByRole("button", { name: "실제 적용값" }).click();
  await page.getByRole("button", { name: "적용 순서 확인" }).click();
  await expect(page.getByText("실제 적용 예상")).toBeVisible();

  await page.screenshot({
    path: "test-results/env-manager-effective.png",
    fullPage: true,
  });
});

test("shows a compact product-style empty project screen", async ({ page }) => {
  await page.goto("/?empty=1");

  await expect(page.getByRole("heading", { name: "프로젝트", exact: true })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "관리할 프로젝트 폴더를 선택하세요" }),
  ).toBeVisible();
  await expect(page.getByText("환경변수는 그대로,")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "폴더 선택…" })).toBeVisible();
  await expect(page.getByText(".env.example")).toBeVisible();

  await page.screenshot({
    path: "test-results/env-manager-empty-project.png",
    fullPage: true,
  });
});
