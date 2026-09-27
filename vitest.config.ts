import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Shared frontend test entry for all remediation packages: any
    // vitest-native suite under tests/ is picked up automatically.
    // remediation-03 and remediation-04 deliberately ship their own
    // zero-dependency runners (node type stripping / node:test + esbuild)
    // with their own entry scripts; vitest must not claim those files,
    // and `pnpm test` invokes their entries separately.
    include: ["tests/**/*.test.ts"],
    exclude: ["tests/remediation-03/**", "tests/remediation-04/**", "**/node_modules/**"],
    environment: "node",
  },
});
