import { execFile } from "node:child_process";
import { mkdir, readFile, rm } from "node:fs/promises";
import { promisify } from "node:util";
import path from "node:path";
import { chromium } from "@playwright/test";

const execFileAsync = promisify(execFile);
const root = process.cwd();
const outputDir = path.join(root, "assets", "jocohunt");
const workDir = path.join(root, "test-results", "jocohunt-assets");
const logoPath = path.join(root, "assets", "brand", "kavranta-logo.svg");
const editorPath = path.join(root, "assets", "screenshots", "kavranta-editor.png");
const codexWorkflowPath = path.join(root, "assets", "screenshots", "kavranta-codex-workflow-ko.png");
const codexDetailImages = [
  ["kavranta-codex-create-reuse", "kavranta-codex-create-reuse-ko.png"],
  ["kavranta-codex-provider-push", "kavranta-codex-provider-push-ko.png"],
  ["kavranta-codex-action-pack", "kavranta-codex-action-pack-ko.png"],
];

await mkdir(outputDir, { recursive: true });
await rm(workDir, { recursive: true, force: true });
await mkdir(workDir, { recursive: true });

const logo = (await readFile(logoPath)).toString("base64");
const editor = (await readFile(editorPath)).toString("base64");
const browser = await chromium.launch();

async function render(name, width, height, markup) {
  const context = await browser.newContext({ viewport: { width, height } });
  const page = await context.newPage();
  await page.setContent(markup);
  const pngPath = path.join(workDir, `${name}.png`);
  await page.screenshot({ path: pngPath });
  await context.close();
  await execFileAsync("cwebp", [
    "-quiet",
    "-q",
    "88",
    "-m",
    "6",
    pngPath,
    "-o",
    path.join(outputDir, `${name}.webp`),
  ]);
}

const baseStyles = `
  * { box-sizing: border-box; }
  html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; }
  body {
    background: #08130f;
    color: #f4f8f6;
    font-family: Inter, Pretendard, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }
`;

try {
  await render("kavranta-app-icon", 512, 512, `
    <!doctype html><html><head><style>
      ${baseStyles}
      body {
        display: grid;
        place-items: center;
        background:
          radial-gradient(circle at 25% 18%, rgba(71, 236, 158, .18), transparent 42%),
          #091712;
      }
      .mark {
        width: 350px;
        height: 350px;
        filter: drop-shadow(0 28px 48px rgba(0, 0, 0, .28));
      }
    </style></head><body>
      <img class="mark" src="data:image/svg+xml;base64,${logo}" alt="" />
    </body></html>
  `);

  await render("kavranta-main", 1600, 1000, `
    <!doctype html><html lang="ko"><head><style>
      ${baseStyles}
      body {
        position: relative;
        background:
          radial-gradient(circle at 8% 4%, rgba(71, 236, 158, .2), transparent 36%),
          linear-gradient(145deg, #0b1b15, #07110d 62%);
      }
      .copy { position: relative; z-index: 2; padding: 62px 76px 0; }
      .brand { display: flex; align-items: center; gap: 14px; font-size: 26px; font-weight: 760; }
      .brand img { width: 55px; height: 55px; }
      h1 { margin: 44px 0 0; font-size: 67px; line-height: 1.05; letter-spacing: -.055em; }
      p { margin: 18px 0 0; color: #b9cac2; font-size: 25px; line-height: 1.45; }
      .window {
        position: absolute;
        top: 326px;
        left: 75px;
        width: 1450px;
        height: 735px;
        overflow: hidden;
        border: 1px solid rgba(255, 255, 255, .2);
        border-radius: 24px;
        background: #eff3f0;
        box-shadow: 0 44px 100px rgba(0, 0, 0, .52);
      }
      .titlebar { height: 42px; display: flex; align-items: center; gap: 9px; padding: 0 17px; background: #20211f; }
      .dot { width: 12px; height: 12px; border-radius: 50%; background: #ff5f57; }
      .dot:nth-child(2) { background: #ffbd2e; }
      .dot:nth-child(3) { background: #28c840; }
      .window img { display: block; width: 1450px; }
    </style></head><body>
      <section class="copy">
        <div class="brand"><img src="data:image/svg+xml;base64,${logo}" alt="" />Kavranta</div>
        <h1>환경변수를 한곳에서 쉽게.</h1>
        <p>찾고 복사하는 대신, 프로젝트의 .env를 한 화면에서 관리하세요.</p>
      </section>
      <section class="window">
        <div class="titlebar"><i class="dot"></i><i class="dot"></i><i class="dot"></i></div>
        <img src="data:image/png;base64,${editor}" alt="" />
      </section>
    </body></html>
  `);

  await execFileAsync("cwebp", [
    "-quiet",
    "-q",
    "88",
    "-m",
    "6",
    editorPath,
    "-o",
    path.join(outputDir, "kavranta-env-editor.webp"),
  ]);

  await execFileAsync("cwebp", [
    "-quiet",
    "-q",
    "88",
    "-m",
    "6",
    codexWorkflowPath,
    "-o",
    path.join(outputDir, "kavranta-codex-workflow.webp"),
  ]);

  for (const [outputName, sourceName] of codexDetailImages) {
    await execFileAsync("cwebp", [
      "-quiet",
      "-q",
      "88",
      "-m",
      "6",
      path.join(root, "assets", "screenshots", sourceName),
      "-o",
      path.join(outputDir, `${outputName}.webp`),
    ]);
  }
} finally {
  await browser.close();
}

console.log(`Rendered JocoHunt assets in ${path.relative(root, outputDir)}`);
