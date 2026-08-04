import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const packageVersion = JSON.parse(await read("package.json")).version;
const tauriVersion = JSON.parse(await read("src-tauri/tauri.conf.json")).version;
const codexVersion = JSON.parse(await read("plugins/env-manager/.codex-plugin/plugin.json")).version;
const claudeVersion = JSON.parse(await read("plugins/env-manager/.claude-plugin/plugin.json")).version;
const workspaceVersion = (await read("Cargo.toml")).match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/)?.[1];

const versions = { packageVersion, tauriVersion, workspaceVersion, codexVersion, claudeVersion };
const mismatched = Object.entries(versions).filter(([, version]) => version !== packageVersion);
if (mismatched.length > 0) {
  throw new Error(`Release versions do not match: ${JSON.stringify(versions)}`);
}

const ref = process.env.GITHUB_REF_NAME;
if (ref?.startsWith("v") && ref !== `v${packageVersion}`) {
  throw new Error(`Tag ${ref} does not match app version v${packageVersion}.`);
}

process.stdout.write(`Release version ${packageVersion} is consistent.\n`);

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
