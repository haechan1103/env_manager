import { expect, test } from "@playwright/test";

test("keeps project selection compact in the sidebar", async ({ page }) => {
  await page.goto("/");

  const sidebar = page.locator(".sidebar");
  await expect(sidebar.getByText("sample-saas", { exact: true })).toBeVisible();
  await expect(sidebar.getByText("PROJECTS", { exact: true })).toHaveCount(0);
  await sidebar.getByRole("button", { name: "Change" }).click();

  const dialog = page.getByRole("dialog", { name: "Switch project" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("/Users/demo/dev/sample-saas")).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Add project" })).toBeVisible();
});

test("navigates the redacted V1 workflow", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Action inbox")).toBeVisible();
  await expect(page.getByText("NEXT_PUBLIC_APP_URL")).toBeVisible();
  await expect(page.getByText("fake_preview_value")).toHaveCount(0);

  await page
    .getByRole("button", { name: /Local environment.*\.env\.local/ })
    .click();
  await expect(page.getByRole("heading", { name: "Local environment" })).toBeVisible();
  await expect(page.getByRole("main").getByText(".env.local", { exact: true }).first()).toBeVisible();
  await page.getByRole("button", { name: "Organize comments" }).click();
  await expect(page.getByRole("heading", { name: "Organize existing env comments" })).toBeVisible();
  await expect(page.getByText("# @group GPT")).toBeVisible();
  await expect(page.getByText("Values are not included in this plan or screen.")).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  const apiKeyInput = page.getByLabel("GPT_API_KEY value");
  await expect(page.getByText("Managed together in 2 files")).toBeVisible();
  await expect(page.getByRole("main").getByText(".env.development")).toBeVisible();
  await page.getByTitle("Show value · hides after 30 seconds of inactivity").first().click();
  await expect(page.locator("textarea.revealed-value-field")).toBeVisible();
  await apiKeyInput.fill("fake_e2e_replacement");
  await expect(page.getByRole("button", { name: "Save to 2 files" })).toBeVisible();
  await page.screenshot({
    path: "test-results/env-manager-file-editor.png",
    fullPage: true,
  });

  await expect(page.getByRole("button", { name: "Effective value" })).toHaveCount(0);
});

test("shows a compact product-style empty project screen", async ({ page }) => {
  await page.goto("/?empty=1");

  await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Choose a project folder" }),
  ).toBeVisible();
  await expect(page.getByText("Environment variables stay where they are")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Choose folder…" })).toBeVisible();
  await expect(page.getByText(".env.example")).toBeVisible();

  await page.screenshot({
    path: "test-results/env-manager-empty-project.png",
    fullPage: true,
  });
});

test("shows one shared integration bundle for supported AI tools", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "AI tool connections" }).click();
  await expect(page.getByRole("heading", { name: "AI tool connections" })).toBeVisible();
  await expect(page.getByText("Codex", { exact: true })).toBeVisible();
  await expect(page.getByText("Claude Code", { exact: true })).toBeVisible();
  await expect(page.getByText("GitHub Copilot / VS Code", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Connect the same rules to every tool you use" })).toBeVisible();
  await expect(page.getByText(/API_KEY=/)).toHaveCount(0);

  await page.screenshot({
    path: "test-results/env-manager-ai-integrations.png",
    fullPage: true,
  });
});

test("persists an explicit Korean language selection", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Items to review" })).toBeVisible();
  await page.getByLabel("Language").selectOption("ko");
  await expect(page.getByRole("heading", { name: "지금 확인할 항목" })).toBeVisible();

  await page.reload();
  await expect(page.getByRole("heading", { name: "지금 확인할 항목" })).toBeVisible();
  await expect(page.getByLabel("언어")).toHaveValue("ko");
});

test("offers complete and variable-level env sharing", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Export" }).click();
  await expect(page.getByRole("heading", { name: "Export env files" })).toBeVisible();
  await expect(page.getByText("Share everything")).toBeVisible();
  await page.getByText("Choose what to share").click();
  await expect(page.getByText("GPT_API_KEY").first()).toBeVisible();
  await expect(page.getByText("Selects 2 linked files together").first()).toBeVisible();
  await expect(page.getByText("fake_preview_value")).toHaveCount(0);
});

test("reviews encrypted-share conflicts individually before applying", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Import share" }).click();
  await page.getByLabel("Share passphrase").fill("fake-team-passphrase-2026");
  await page.getByRole("button", { name: "Choose encrypted file" }).click();

  await expect(page.getByText("Choose where each file goes")).toBeVisible();
  await expect(page.getByText("Linked across 2 files")).toBeVisible();
  await expect(page.getByText("Use received 0")).toBeVisible();
  await expect(page.getByText("fake_local_value")).toHaveCount(0);

  const publicConflict = page.locator(".import-conflict-card").filter({ hasText: "VITE_API_BASE_URL" });
  await publicConflict.getByRole("button", { name: "Reveal my local value" }).click();
  await expect(publicConflict.getByText("fake_local_value")).toBeVisible();
  await publicConflict.getByRole("button", { name: "Use shared" }).click();
  await expect(page.getByText("Use received 1")).toBeVisible();

  await publicConflict.getByRole("button", { name: "Hide value" }).click();
  const webTarget = page.getByLabel("Target file for apps/web/.env.local");
  await webTarget.fill("apps/web/.env.staging");
  await webTarget.locator("..").getByRole("button", { name: "Change" }).click();
  await expect(webTarget).toHaveValue("apps/web/.env.staging");
  await expect(page.locator(".import-summary .conflict strong")).toHaveText("2");

  await page.getByRole("button", { name: "Use all shared" }).click();
  await expect(page.getByText("Use received 2")).toBeVisible();
  await expect(page.getByText("fake_local_value")).toHaveCount(0);
  await page.screenshot({
    path: "test-results/env-manager-import-conflicts.png",
    fullPage: true,
  });
});
