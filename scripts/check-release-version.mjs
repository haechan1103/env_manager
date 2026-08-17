import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const packageVersion = JSON.parse(await read("package.json")).version;
const packageLock = JSON.parse(await read("package-lock.json"));
const tauriVersion = JSON.parse(await read("src-tauri/tauri.conf.json")).version;
const workspaceVersion = (await read("Cargo.toml")).match(/\[workspace\.package\][\s\S]*?version = "([^"]+)"/)?.[1];

const versions = {
  packageVersion,
  packageLockVersion: packageLock.version,
  packageLockRootVersion: packageLock.packages?.[""]?.version,
  tauriVersion,
  workspaceVersion,
};
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
