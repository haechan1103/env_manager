import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const workflow = await read(".github/workflows/release.yml");
const signedConfig = JSON.parse(await read("src-tauri/tauri.release.conf.json"));
const windowsBetaConfig = JSON.parse(
  await read("src-tauri/tauri.windows-beta.conf.json"),
);

assert(
  signedConfig.bundle?.windows?.signCommand,
  "The signed release configuration must keep a Windows signCommand for future trusted releases.",
);
assert(
  !windowsBetaConfig.bundle?.windows?.signCommand,
  "The Windows beta configuration must not contain a platform signing command.",
);
assert(
  windowsBetaConfig.bundle?.createUpdaterArtifacts === true,
  "The Windows beta must still create Tauri-signed updater artifacts.",
);
assert(
  /^  windows-beta:/m.test(workflow),
  "The release workflow must expose the unsigned Windows job as windows-beta.",
);
assert(
  workflow.includes("--config src-tauri/tauri.windows-beta.conf.json"),
  "The Windows beta job must use its isolated Tauri configuration.",
);
assert(
  workflow.includes("Windows 10/11 (x64 beta, unsigned)"),
  "Release notes must explicitly disclose the unsigned Windows beta.",
);
assert(
  workflow.includes("Windows 10/11(x64 베타, 미서명)"),
  "Korean release notes must explicitly disclose the unsigned Windows beta.",
);
assert(
  workflow.includes("needs: [macos, windows-beta]"),
  "Publishing must wait for both signed macOS and Windows beta jobs.",
);

process.stdout.write("Release distribution policy is consistent.\n");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}
