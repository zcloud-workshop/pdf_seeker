import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Unit-test entry for all remediation packages: any
    // tests/remediation-NN/*.test.ts file is picked up automatically.
    include: ["tests/**/*.test.ts"],
    environment: "node",
  },
});
