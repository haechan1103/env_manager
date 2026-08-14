import { execFile } from "node:child_process";
import { mkdir, readFile, rm } from "node:fs/promises";
import { promisify } from "node:util";
import path from "node:path";
import { chromium } from "@playwright/test";

const execFileAsync = promisify(execFile);
const root = process.cwd();
const baseUrl = process.env.ENV_MANAGER_MEDIA_URL ?? "http://127.0.0.1:1420";
const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const outputDir = path.join(root, "assets", "screenshots");
const brandDir = path.join(root, "assets", "brand");
const workDir = path.join(root, "test-results", "readme-media");
const overviewPath = path.join(outputDir, "env-manager-overview.png");
const heroPath = path.join(outputDir, "env-manager-editor.png");
const integrationsPath = path.join(outputDir, "env-manager-ai-integrations.png");
const providerPath = path.join(outputDir, "env-manager-cloudflare-push.png");
const sharingPath = path.join(outputDir, "env-manager-encrypted-share.png");
const gifPath = path.join(outputDir, "env-manager-demo.gif");
const socialPreviewPath = path.join(brandDir, "env-manager-social-preview.png");

await mkdir(outputDir, { recursive: true });
await mkdir(brandDir, { recursive: true });
await rm(workDir, { recursive: true, force: true });
await mkdir(workDir, { recursive: true });

const browser = await chromium.launch({ executablePath: chromePath });

async function captureScreenshots() {
  const context = await browser.newContext({ viewport: { width: 1440, height: 940 } });
  const page = await context.newPage();
  await page.goto(baseUrl);
  await page.getByText("Action inbox").waitFor();
  await page.screenshot({ path: overviewPath, fullPage: true });

  await page.getByRole("button", { name: /Local environment.*\.env\.local/ }).click();
  await page.getByRole("heading", { name: "Local environment" }).waitFor();
  await page.screenshot({ path: heroPath, fullPage: true });

  await page.getByRole("button", { name: "Push variables" }).click();
  await page.getByRole("heading", { name: "Push variables" }).waitFor();
  await page.getByRole("button", { name: /Cloudflare Workers/ }).click();
  await page.getByLabel("Worker").fill("sample-worker");
  await page.getByRole("button", { name: "Select all" }).click();
  await page.screenshot({ path: providerPath, fullPage: true });
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "Export" }).click();
  await page.getByRole("heading", { name: "Export env files" }).waitFor();
  await page.getByText("Choose what to share").click();
  await page.getByRole("dialog").locator(".share-variable-select").filter({ hasText: "GPT_API_KEY" }).first().click();
  await page.screenshot({ path: sharingPath, fullPage: true });
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "AI tool connections" }).click();
  await page.getByRole("heading", { name: "AI tool connections" }).waitFor();
  await page.screenshot({ path: integrationsPath, fullPage: true });
  await context.close();
}

async function captureDemoFrames() {
  const context = await browser.newContext({ viewport: { width: 1200, height: 780 } });
  const page = await context.newPage();
  let frame = 0;
  const capture = async (count) => {
    for (let index = 0; index < count; index += 1) {
      frame += 1;
      await page.screenshot({
        path: path.join(workDir, `frame-${String(frame).padStart(3, "0")}.png`),
      });
    }
  };

  await page.goto(baseUrl);
  await page.getByText("Action inbox").waitFor();
  await capture(6);
  await page.getByRole("button", { name: /Local environment.*\.env\.local/ }).hover();
  await capture(2);
  await page.getByRole("button", { name: /Local environment.*\.env\.local/ }).click();
  await page.getByRole("heading", { name: "Local environment" }).waitFor();
  await capture(6);
  await page.getByRole("button", { name: "Push variables" }).click();
  await page.getByRole("heading", { name: "Push variables" }).waitFor();
  await page.getByRole("button", { name: /Cloudflare Workers/ }).click();
  await capture(6);
  await page.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: "AI tool connections" }).hover();
  await capture(2);
  await page.getByRole("button", { name: "AI tool connections" }).click();
  await page.getByRole("heading", { name: "AI tool connections" }).waitFor();
  await capture(6);
  const overviewButton = page.locator('nav[aria-label="Project views"] button').first();
  await overviewButton.hover();
  await capture(2);
  await overviewButton.click();
  await page.getByText("Action inbox").waitFor();
  await capture(6);

  await context.close();
}

async function renderSocialPreview() {
  const appImage = (await readFile(heroPath)).toString("base64");
  const logoImage = (
    await readFile(path.join(brandDir, "env-manager-logo-v1.png"))
  ).toString("base64");
  const context = await browser.newContext({ viewport: { width: 1280, height: 640 } });
  const page = await context.newPage();
  await page.setContent(`
    <!doctype html>
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <style>
          * { box-sizing: border-box; }
          body {
            margin: 0;
            width: 1280px;
            height: 640px;
            overflow: hidden;
            background:
              radial-gradient(circle at 14% 12%, rgba(60, 213, 166, .18), transparent 34%),
              #0b1713;
            color: #f5f8f6;
            font-family: Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
          }
          .canvas { position: relative; width: 100%; height: 100%; padding: 72px 66px; }
          .brand { display: flex; align-items: center; gap: 15px; }
          .logo {
            width: 64px; height: 64px; object-fit: cover; border-radius: 17px;
            box-shadow: 0 10px 35px rgba(0, 0, 0, .22);
          }
          .name { font-size: 24px; font-weight: 760; letter-spacing: -.02em; }
          .copy { position: relative; z-index: 2; width: 535px; margin-top: 68px; }
          h1 { margin: 0; font-size: 55px; line-height: 1.04; letter-spacing: -.055em; }
          p { margin: 24px 0 0; width: 490px; color: #b9c8c1; font-size: 23px; line-height: 1.38; }
          .chips { display: flex; gap: 10px; margin-top: 34px; }
          .chip {
            border: 1px solid rgba(117, 233, 195, .25); border-radius: 999px;
            padding: 9px 14px; color: #b9f5df; background: rgba(53, 208, 160, .08);
            font-size: 15px; font-weight: 650;
          }
          .app {
            position: absolute; left: 622px; top: 91px; width: 790px; height: 512px;
            overflow: hidden; border-radius: 18px; border: 1px solid rgba(255,255,255,.16);
            background: #eef2ef; box-shadow: 0 34px 80px rgba(0,0,0,.48);
            transform: rotate(-1.5deg);
          }
          .bar { height: 34px; background: #20211f; display: flex; align-items: center; gap: 8px; padding-left: 14px; }
          .dot { width: 10px; height: 10px; border-radius: 50%; background: #ff5f57; }
          .dot:nth-child(2) { background: #ffbd2e; }
          .dot:nth-child(3) { background: #28c840; }
          .app img { width: 790px; display: block; }
        </style>
      </head>
      <body>
        <main class="canvas">
          <div class="brand">
            <img class="logo" src="data:image/png;base64,${logoImage}" />
            <span class="name">Env Manager</span>
          </div>
          <section class="copy">
            <h1>Edit. Share.<br />Deploy your .env.</h1>
            <p>Manage local env files, work with AI agents, and push selected values without exposing everything.</p>
            <div class="chips">
              <span class="chip">Local-first</span>
              <span class="chip">GitHub Actions</span>
              <span class="chip">Cloudflare</span>
            </div>
          </section>
          <section class="app">
            <div class="bar"><i class="dot"></i><i class="dot"></i><i class="dot"></i></div>
            <img src="data:image/png;base64,${appImage}" />
          </section>
        </main>
      </body>
    </html>
  `);
  await page.screenshot({ path: socialPreviewPath });
  await context.close();
}

try {
  await captureScreenshots();
  await captureDemoFrames();
  await renderSocialPreview();
} finally {
  await browser.close();
}

await execFileAsync("ffmpeg", [
  "-y",
  "-framerate",
  "8",
  "-i",
  path.join(workDir, "frame-%03d.png"),
  "-filter_complex",
  "scale=960:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
  "-loop",
  "0",
  gifPath,
]);

console.log(`Captured README media in ${path.relative(root, outputDir)}`);
console.log(`Rendered social preview at ${path.relative(root, socialPreviewPath)}`);
