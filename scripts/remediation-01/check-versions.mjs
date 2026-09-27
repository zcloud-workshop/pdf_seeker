#!/usr/bin/env node
// Verify that a release tag matches the versions declared in
// package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json.
//
// Used by the release workflow before any artifact is published: a
// mismatch aborts the run with a readable error and no release side
// effects. Also runnable locally: node scripts/remediation-01/check-versions.mjs v0.4.0
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const TAG_PATTERN = /^v\d+\.\d+\.\d+(?:-[\w.-]+)?$/;

export function normalizeTag(tag) {
  if (typeof tag !== "string" || !TAG_PATTERN.test(tag.trim())) {
    throw new Error(
      `release tag must look like vMAJOR.MINOR.PATCH (optionally with a pre-release), got: ${JSON.stringify(tag)}`,
    );
  }
  return tag.trim().replace(/^v/, "");
}

// Dependency tables also contain `version = "..."` lines, so only the
// `version` directly inside [package] counts.
export function parseCargoPackageVersion(tomlText) {
  let inPackage = false;
  for (const line of tomlText.split(/\r?\n/)) {
    const section = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (section) {
      inPackage = section[1].trim() === "package";
      continue;
    }
    if (!inPackage) continue;
    const match = line.match(/^\s*version\s*=\s*"([^"]+)"/);
    if (match) return match[1];
  }
  throw new Error("no [package] version found in Cargo.toml");
}

export function readDeclaredVersions(root) {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const conf = JSON.parse(
    readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  const cargoToml = readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8");
  return {
    package: pkg.version,
    cargo: parseCargoPackageVersion(cargoToml),
    tauri: conf.version,
  };
}

export function checkVersions(root, tag) {
  const expected = normalizeTag(tag);
  const declared = readDeclaredVersions(root);
  const mismatches = Object.entries(declared)
    .filter(([, version]) => version !== expected)
    .map(([source, version]) => `${source} declares ${version}, expected ${expected}`);
  return { expected, declared, mismatches };
}

function main() {
  const [tag, rootArg] = process.argv.slice(2);
  if (!tag) {
    console.error("usage: node scripts/remediation-01/check-versions.mjs <tag> [repo-root]");
    process.exit(2);
  }
  const root = rootArg ?? join(dirname(fileURLToPath(import.meta.url)), "..", "..");
  const { expected, declared, mismatches } = checkVersions(root, tag);
  if (mismatches.length > 0) {
    console.error(`check-versions: tag v${expected} does not match the declared versions:`);
    for (const mismatch of mismatches) {
      console.error(`  - ${mismatch}`);
    }
    process.exit(1);
  }
  console.log(
    `check-versions: tag v${expected} matches package/cargo/tauri version ${declared.package}`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
