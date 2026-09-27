import { readFileSync } from "node:fs";
import { parse } from "svelte/compiler";
import { describe, expect, it } from "vitest";
import rustS3ListSample from "./s3-list-result.json";
import {
  parseS3ListResult,
  parseS3VersionsResult,
} from "../../src/lib/storage/contracts";

describe("S3 IPC contracts", () => {
  it("parses the Rust-serialized snake_case list fixture", () => {
    expect(parseS3ListResult(rustS3ListSample)).toEqual(rustS3ListSample);
  });

  it("requires the Rust version field names", () => {
    const sample = {
      versions: [
        {
          version_id: "v1",
          size: 7,
          last_modified: "2026-09-27T12:00:00Z",
          is_latest: true,
        },
      ],
      delete_markers: ["d1"],
    };
    expect(parseS3VersionsResult(sample)).toEqual(sample);
    expect(() =>
      parseS3VersionsResult({
        versions: [
          {
            versionId: "v1",
            size: 7,
            lastModified: "2026-09-27T12:00:00Z",
            isLatest: true,
          },
        ],
        deleteMarkers: [],
      }),
    ).toThrow();
  });
});

describe("Input component contract", () => {
  it("binds native value and forwards the remaining input attributes", () => {
    const source = readFileSync(
      new URL("../../src/lib/components/ui/Input.svelte", import.meta.url),
      "utf8",
    );
    const input = parse(source).html.children.find(
      (node) => node.type === "Element" && node.name === "input",
    );
    expect(input?.type).toBe("Element");
    if (input?.type !== "Element") return;
    expect(
      input.attributes.some(
        (attribute) => attribute.type === "Binding" && attribute.name === "value",
      ),
    ).toBe(true);
    expect(input.attributes.some((attribute) => attribute.type === "Spread")).toBe(true);
  });
});
