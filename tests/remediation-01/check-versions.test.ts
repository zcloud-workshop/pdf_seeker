import { describe, expect, it } from "vitest";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  checkVersions,
  normalizeTag,
  parseCargoPackageVersion,
  readDeclaredVersions,
} from "../../scripts/remediation-01/check-versions.mjs";

describe("normalizeTag", () => {
  it("strips the v prefix from well-formed release tags", () => {
    expect(normalizeTag("v0.4.0")).toBe("0.4.0");
    expect(normalizeTag(" v1.2.3 ")).toBe("1.2.3");
    expect(normalizeTag("v1.2.3-rc.1")).toBe("1.2.3-rc.1");
  });

  it("rejects tags that are not vMAJOR.MINOR.PATCH", () => {
    for (const bad of ["0.4.0", "v0.4", "v0.4.0.0", "release", "", null, 42]) {
      expect(() => normalizeTag(bad as string)).toThrow(/release tag/);
    }
  });
});

describe("parseCargoPackageVersion", () => {
  it("reads the [package] version and ignores dependency version lines", () => {
    const toml = [
      "[package]",
      'name = "pdf_seeker"',
      'version = "0.4.0"',
      "",
      "[dependencies]",
      'serde = { version = "1", features = ["derive"] }',
      "",
      "[dev-dependencies]",
      'tempfile = "3"',
    ].join("\n");
    expect(parseCargoPackageVersion(toml)).toBe("0.4.0");
  });

  it("fails loudly when no [package] version exists", () => {
    expect(() => parseCargoPackageVersion("[dependencies]\nserde = '1'\n")).toThrow(
      /no \[package\] version/,
    );
  });
});

function fixtureRepo(versions: { package: string; cargo: string; tauri: string }) {
  const root = mkdtempSync(join(tmpdir(), "check-versions-"));
  mkdirSync(join(root, "src-tauri"), { recursive: true });
  writeFileSync(join(root, "package.json"), JSON.stringify({ version: versions.package }));
  writeFileSync(
    join(root, "src-tauri", "Cargo.toml"),
    `[package]\nversion = "${versions.cargo}"\n`,
  );
  writeFileSync(
    join(root, "src-tauri", "tauri.conf.json"),
    JSON.stringify({ version: versions.tauri }),
  );
  return root;
}

describe("checkVersions", () => {
  it("accepts a tag that matches all three declared versions", () => {
    const root = fixtureRepo({ package: "0.4.0", cargo: "0.4.0", tauri: "0.4.0" });
    try {
      const result = checkVersions(root, "v0.4.0");
      expect(result.mismatches).toEqual([]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("reports every declared version that disagrees with the tag", () => {
    const root = fixtureRepo({ package: "0.4.0", cargo: "0.3.9", tauri: "0.5.0" });
    try {
      const { mismatches } = checkVersions(root, "v0.4.0");
      expect(mismatches).toHaveLength(2);
      expect(mismatches.join("\n")).toContain("cargo declares 0.3.9");
      expect(mismatches.join("\n")).toContain("tauri declares 0.5.0");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses malformed tags before reading any file", () => {
    const root = fixtureRepo({ package: "0.4.0", cargo: "0.4.0", tauri: "0.4.0" });
    try {
      expect(() => checkVersions(root, "v0.1.1234-nope+")).toThrow(/release tag/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("repository standing check", () => {
  it("keeps package.json, Cargo.toml and tauri.conf.json on one version", () => {
    const declared = readDeclaredVersions(process.cwd());
    expect(declared.cargo).toBe(declared.package);
    expect(declared.tauri).toBe(declared.package);
  });
});
