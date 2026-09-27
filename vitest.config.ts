import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "path";
import { defineConfig } from "vitest/config";

export default defineConfig({
  // Mirror vite.config.ts so rune modules (.svelte.ts session/stores) and
  // "@" / "$views" imports resolve the same way under test (07-A).
  plugins: [svelte()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src/lib"),
      "$views": resolve(__dirname, "src/views"),
    },
  },
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
