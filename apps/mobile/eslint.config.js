// https://docs.expo.dev/guides/using-eslint/
const { defineConfig } = require("eslint/config");
const expoConfig = require("eslint-config-expo/flat");

module.exports = defineConfig([
  expoConfig,
  {
    ignores: ["dist/*"],
    rules: {
      // No explicit `any`. Strictness catches most of it; this closes the
      // deliberate escape hatch (the ticket asks for no `any` in domain types).
      "@typescript-eslint/no-explicit-any": "error",
    },
  },
]);
