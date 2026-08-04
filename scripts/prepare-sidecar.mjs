import { chmod, copyFile, mkdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const targetTriple = process.env.ENV_MANAGER_TARGET || commandOutput("rustc", ["--print", "host-tuple"]);
const isWindows = targetTriple.includes("windows");
const executableName = isWindows ? "env-manager-broker.exe" : "env-manager-broker";
const cargoArguments = ["build", "--release", "--locked", "-p", "env-broker"];

if (process.env.ENV_MANAGER_TARGET) {
  cargoArguments.push("--target", targetTriple);
}

run("cargo", cargoArguments);

const source = process.env.ENV_MANAGER_TARGET
  ? join(repositoryRoot, "target", targetTriple, "release", executableName)
  : join(repositoryRoot, "target", "release", executableName);
const destinationName = isWindows
  ? `env-manager-broker-${targetTriple}.exe`
  : `env-manager-broker-${targetTriple}`;
const destination = join(repositoryRoot, "src-tauri", "binaries", destinationName);

await mkdir(dirname(destination), { recursive: true });
await copyFile(source, destination);
if (!isWindows) await chmod(destination, 0o755);

process.stdout.write(`Prepared Env Manager broker sidecar for ${targetTriple}.\n`);

function commandOutput(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
  return result.stdout.trim();
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repositoryRoot,
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
