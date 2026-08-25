import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const path = resolve(root, "config/provider-compatibility.json");
const catalog = JSON.parse(await readFile(path, "utf8"));

const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const identifier = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const date = /^\d{4}-\d{2}-\d{2}$/;
const supportStates = new Set(["implemented", "planned"]);
const transports = new Set(["cli", "cli-pty", "sdk", "remote-verifier"]);
const valueTransports = new Set(["stdin", "hidden-interactive-prompt", "in-process-sdk", "age-stdin"]);
const runtimeProbes = new Set(["semantic-version", "not-implemented", "sdk-config-chain", "fixed-protocol"]);
const strategies = new Set(["gh-secret-set-v1", "wrangler-secret-bulk-v1", "eas-env-set-prompt-v1"]);

exactKeys(catalog, [
  "schemaVersion",
  "catalogVersion",
  "providerProtocolVersion",
  "lastReviewed",
  "providers",
], "catalog");
assert(catalog.schemaVersion === 2, "Provider catalog schemaVersion must be 2");
assert(semver.test(catalog.catalogVersion), "catalogVersion must be semantic");
assert(semver.test(catalog.providerProtocolVersion), "providerProtocolVersion must be semantic");
assert(date.test(catalog.lastReviewed), "lastReviewed must use YYYY-MM-DD");
assert(Array.isArray(catalog.providers) && catalog.providers.length > 0, "providers must be a non-empty array");

const providerIds = new Set();
for (const provider of catalog.providers) {
  exactKeys(provider, [
    "id",
    "displayName",
    "adapterVersion",
    "uiSupport",
    "agentSupport",
    "transport",
    "client",
    "valueTransport",
    "capabilities",
    "officialDocs",
  ], `provider ${provider.id ?? "<missing>"}`);
  assert(identifier.test(provider.id), `Invalid provider id: ${provider.id}`);
  assert(!providerIds.has(provider.id), `Duplicate provider id: ${provider.id}`);
  providerIds.add(provider.id);
  assert(typeof provider.displayName === "string" && provider.displayName.trim().length > 0, `${provider.id} needs displayName`);
  assert(supportStates.has(provider.uiSupport), `${provider.id} has invalid uiSupport`);
  assert(supportStates.has(provider.agentSupport), `${provider.id} has invalid agentSupport`);
  assert(transports.has(provider.transport), `${provider.id} has invalid transport`);
  assert(valueTransports.has(provider.valueTransport), `${provider.id} has invalid valueTransport`);

  const implemented = provider.uiSupport === "implemented" || provider.agentSupport === "implemented";
  assert(
    implemented ? semver.test(provider.adapterVersion) : provider.adapterVersion === null || semver.test(provider.adapterVersion),
    `${provider.id} must have a semantic adapterVersion once implemented`,
  );

  exactKeys(provider.client, ["name", "runtimeProbe", "profiles"], `${provider.id}.client`);
  assert(identifier.test(provider.client.name), `${provider.id} has invalid client name`);
  assert(runtimeProbes.has(provider.client.runtimeProbe), `${provider.id} has invalid runtimeProbe`);
  assert(Array.isArray(provider.client.profiles), `${provider.id} profiles must be an array`);
  const profileIds = new Set();
  for (const profile of provider.client.profiles) {
    exactKeys(profile, ["id", "strategy", "versionRequirement"], `${provider.id}.profile`);
    assert(identifier.test(profile.id), `${provider.id} has invalid profile id`);
    assert(!profileIds.has(profile.id), `${provider.id} has duplicate profile ${profile.id}`);
    profileIds.add(profile.id);
    assert(strategies.has(profile.strategy), `${provider.id} has unknown strategy ${profile.strategy}`);
    assert(typeof profile.versionRequirement === "string" && profile.versionRequirement.length > 0, `${provider.id} profile needs a version requirement`);
  }

  if (provider.transport === "cli" || provider.transport === "cli-pty") {
    const expectedValueTransport = provider.transport === "cli" ? "stdin" : "hidden-interactive-prompt";
    assert(provider.valueTransport === expectedValueTransport, `${provider.id} CLI value transport does not match its process transport`);
    assert(provider.client.runtimeProbe !== "not-implemented", `${provider.id} implemented CLI needs a runtime probe`);
    assert(provider.client.profiles.length > 0, `${provider.id} implemented CLI needs a compatibility profile`);
  } else if (provider.transport === "sdk") {
    assert(provider.valueTransport === "in-process-sdk", `${provider.id} SDK values must stay in process`);
    assert(provider.client.profiles.length === 0, `${provider.id} SDK must not claim a CLI profile`);
    assert(
      implemented ? provider.client.runtimeProbe === "sdk-config-chain" : provider.client.runtimeProbe === "not-implemented",
      `${provider.id} SDK support state and runtime probe disagree`,
    );
  } else {
    assert(provider.valueTransport === "age-stdin", `${provider.id} remote values must use age-encrypted stdin`);
    assert(provider.client.runtimeProbe === "fixed-protocol", `${provider.id} must pin a fixed protocol`);
    assert(provider.client.profiles.length === 0, `${provider.id} remote verifier must not use CLI profiles`);
  }

  assert(Array.isArray(provider.capabilities) && provider.capabilities.length > 0, `${provider.id} needs capabilities`);
  assert(provider.capabilities.every((capability) => identifier.test(capability)), `${provider.id} has invalid capability`);
  assert(new Set(provider.capabilities).size === provider.capabilities.length, `${provider.id} capabilities must be unique`);
  assert(Array.isArray(provider.officialDocs) && provider.officialDocs.length > 0, `${provider.id} needs officialDocs`);
  assert(provider.officialDocs.every(isHttpsUrl), `${provider.id} officialDocs must use HTTPS`);
}

process.stdout.write(
  `Provider catalog ${catalog.catalogVersion} is valid for protocol ${catalog.providerProtocolVersion} with ${catalog.providers.length} providers.\n`,
);

function exactKeys(value, expected, label) {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const actual = Object.keys(value).sort();
  const allowed = [...expected].sort();
  assert(JSON.stringify(actual) === JSON.stringify(allowed), `${label} has unexpected or missing fields`);
}

function isHttpsUrl(value) {
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
