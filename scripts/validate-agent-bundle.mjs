import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const version = JSON.parse(await read("package.json")).version;
const codex = JSON.parse(await read("plugins/env-manager/.codex-plugin/plugin.json"));
const claude = JSON.parse(await read("plugins/env-manager/.claude-plugin/plugin.json"));
const claudeMarketplace = JSON.parse(await read(".claude-plugin/marketplace.json"));
const mcp = JSON.parse(await read("plugins/env-manager/.mcp.json"));
const hooks = JSON.parse(await read("plugins/env-manager/hooks/hooks.json"));
const skill = await read("plugins/env-manager/skills/manage-project-env/SKILL.md");

assert(codex.name === "env-manager", "Codex plugin name must be env-manager");
assert(claude.name === "env-manager", "Claude plugin name must be env-manager");
assert(codex.version === version, "Codex plugin version must match the app");
assert(claude.version === version, "Claude plugin version must match the app");
assert(claudeMarketplace.plugins?.[0]?.version === version, "Claude marketplace version must match the app");
assert(mcp.mcpServers?.["env-manager"]?.command === "env-manager-broker", "MCP must use the portable broker command");
assert(Array.isArray(hooks.hooks?.PreToolUse), "PreToolUse Guard is required");
assert(skill.startsWith("---\nname: manage-project-env\n"), "Skill frontmatter is missing");

process.stdout.write(`Agent bundle ${version} is consistent.\n`);

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
