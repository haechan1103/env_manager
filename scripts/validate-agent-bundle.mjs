import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const version = (await read("plugins/env-manager/VERSION")).trim();
const codex = JSON.parse(await read("plugins/env-manager/.codex-plugin/plugin.json"));
const claude = JSON.parse(await read("plugins/env-manager/.claude-plugin/plugin.json"));
const codexMarketplace = JSON.parse(await read(".agents/plugins/marketplace.json"));
const claudeMarketplace = JSON.parse(await read(".claude-plugin/marketplace.json"));
const mcp = JSON.parse(await read("plugins/env-manager/.mcp.json"));
const hooks = JSON.parse(await read("plugins/env-manager/hooks/hooks.json"));
const skill = await read("plugins/env-manager/skills/manage-project-env/SKILL.md");
const normalizedSkill = skill.replace(/\r\n?/g, "\n");
const productName = "Kavranta";
const repository = "https://github.com/haechan1103/kavranta";

assert(codex.name === "env-manager", "Codex plugin name must be env-manager");
assert(claude.name === "env-manager", "Claude plugin name must be env-manager");
assert(
  codex.interface?.displayName === productName,
  `Codex plugin display name must be ${productName}`,
);
assert(codex.repository === repository, "Codex plugin must reference the Kavranta repository");
assert(claude.repository === repository, "Claude plugin must reference the Kavranta repository");
assert(
  codexMarketplace.interface?.displayName === productName,
  `Codex marketplace display name must be ${productName}`,
);
assert(
  codex.interface?.defaultPrompt?.length <= 3,
  "Codex plugin must expose at most three starter prompts",
);
assert(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version), "Agent bundle version must be semantic");
assert(codex.version === version, "Codex plugin version must match the agent bundle");
assert(claude.version === version, "Claude plugin version must match the agent bundle");
assert(claudeMarketplace.plugins?.[0]?.version === version, "Claude marketplace version must match the agent bundle");
assert(mcp.mcpServers?.["env-manager"]?.command === "env-manager-broker", "MCP must use the portable broker command");
assert(Array.isArray(hooks.hooks?.PreToolUse), "PreToolUse Guard is required");
assert(normalizedSkill.startsWith("---\nname: manage-project-env\n"), "Skill frontmatter is missing");
assert(normalizedSkill.includes("plan_register_current_project"), "Skill must route safe current-project registration");
assert(normalizedSkill.includes("find_reusable_variable_sources"), "Skill must route redacted cross-project source discovery");
assert(normalizedSkill.includes("plan_copy_variable_from_project"), "Skill must route opaque cross-project copy plans");
assert(normalizedSkill.includes("plan_provider_push"), "Skill must route opaque provider push plans");
assert(normalizedSkill.includes("personal-provider-packs.md"), "Skill must route Personal Provider Pack authoring");
assert(normalizedSkill.includes("list_action_packs"), "Skill must route installed Action Pack discovery");
assert(normalizedSkill.includes("plan_action"), "Skill must route opaque Action Pack plans");
assert(normalizedSkill.includes("action-packs.md"), "Skill must route Action Pack authoring");
assert(normalizedSkill.includes("plan_stdin_value_write"), "Skill must route opaque stdin value plans");
assert(normalizedSkill.includes("stdin-value-ingest.md"), "Skill must route opaque stdin value guidance");
assert(normalizedSkill.includes(".dev.vars"), "Skill must route Wrangler .dev.vars files through the broker");

process.stdout.write(`Agent bundle ${version} is internally consistent and versioned independently from the app.\n`);

async function read(path) {
  return readFile(resolve(root, path), "utf8");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
