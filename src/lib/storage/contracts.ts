export interface S3FileItem {
  key: string;
  name: string;
  size: number;
  last_modified: string;
  is_dir: boolean;
}

export interface S3ListResult {
  items: S3FileItem[];
  prefixes: string[];
  common_prefixes: string[];
}

export interface S3VersionItem {
  version_id: string;
  size: number;
  last_modified: string;
  is_latest: boolean;
}

export interface S3VersionsResult {
  versions: S3VersionItem[];
  delete_markers: string[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(value: unknown, field: string): string {
  if (typeof value !== "string") throw new Error("Invalid S3 response field: " + field);
  return value;
}

function requireNumber(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error("Invalid S3 response field: " + field);
  }
  return value;
}

export function parseS3ListResult(value: unknown): S3ListResult {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("Invalid S3 list response");
  }
  const items = value.items.map((item) => {
    if (!isRecord(item) || typeof item.is_dir !== "boolean") {
      throw new Error("Invalid S3 list item");
    }
    return {
      key: requireString(item.key, "key"),
      name: requireString(item.name, "name"),
      size: requireNumber(item.size, "size"),
      last_modified: requireString(item.last_modified, "last_modified"),
      is_dir: item.is_dir,
    };
  });
  const prefixes = value.prefixes;
  const commonPrefixes = value.common_prefixes;
  if (
    !Array.isArray(prefixes) ||
    !prefixes.every((prefix) => typeof prefix === "string") ||
    !Array.isArray(commonPrefixes) ||
    !commonPrefixes.every((prefix) => typeof prefix === "string")
  ) {
    throw new Error("Invalid S3 list prefixes");
  }
  return { items, prefixes, common_prefixes: commonPrefixes };
}

export function parseS3VersionsResult(value: unknown): S3VersionsResult {
  if (!isRecord(value) || !Array.isArray(value.versions)) {
    throw new Error("Invalid S3 versions response");
  }
  const versions = value.versions.map((version) => {
    if (!isRecord(version) || typeof version.is_latest !== "boolean") {
      throw new Error("Invalid S3 version item");
    }
    return {
      version_id: requireString(version.version_id, "version_id"),
      size: requireNumber(version.size, "size"),
      last_modified: requireString(version.last_modified, "last_modified"),
      is_latest: version.is_latest,
    };
  });
  const deleteMarkers = value.delete_markers;
  if (
    !Array.isArray(deleteMarkers) ||
    !deleteMarkers.every((marker) => typeof marker === "string")
  ) {
    throw new Error("Invalid S3 delete markers");
  }
  return { versions, delete_markers: deleteMarkers };
}
