const eslint = require("@eslint/js");
const angular = require("angular-eslint");
const tseslint = require("typescript-eslint");
const perfectionist = require("eslint-plugin-perfectionist");

module.exports = tseslint.config(
  { ignores: ["dist/**", "src-tauri/**"] },
  {
    files: ["**/*.ts"],
    extends: [
      eslint.configs.recommended,
      ...tseslint.configs.recommended,
      ...tseslint.configs.stylistic,
      ...angular.configs.tsRecommended,
    ],
    processor: angular.processInlineTemplates,
    rules: {
      "@angular-eslint/component-selector": [
        "error",
        { type: "element", prefix: "app", style: "kebab-case" },
      ],
      "@angular-eslint/directive-selector": [
        "error",
        { type: "attribute", prefix: "app", style: "camelCase" },
      ],
      "@angular-eslint/prefer-on-push-component-change-detection": [
        "error",
        { allowExplicitOnPush: false },
      ],
      "@typescript-eslint/array-type": "off",
      "@typescript-eslint/no-empty-function": ["error", { allow: ["arrowFunctions"] }],
      "eqeqeq": ["error", "always", { null: "ignore" }],
      "no-empty": ["error", { allowEmptyCatch: true }],
      "no-console": "error",
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/naming-convention": [
        "error",
        { selector: "default", format: ["camelCase"], leadingUnderscore: "allow" },
        { selector: "typeLike", format: ["PascalCase"] },
        { selector: "enumMember", format: ["UPPER_CASE", "PascalCase"] },
        { selector: "import", format: ["camelCase", "PascalCase"] },
        { selector: "objectLiteralProperty", format: null },
        { selector: "typeProperty", format: null },
        {
          selector: "variable",
          modifiers: ["const"],
          format: ["camelCase", "UPPER_CASE", "PascalCase"],
        },
        { selector: "classProperty", modifiers: ["static"], format: ["UPPER_CASE", "camelCase"] },
        { selector: "parameter", format: ["camelCase"], leadingUnderscore: "allow" },
      ],
    },
  },
  {
    files: ["**/*.ts"],
    plugins: { perfectionist },
    rules: {
      "perfectionist/sort-imports": [
        "error",
        {
          type: "line-length",
          order: "asc",
          newlinesBetween: 1,
          groups: [
            "side-effect",
            ["type-builtin", "type-external"],
            ["builtin", "external"],
            "env-type",
            "env",
            "models-type",
            "models",
            "constants-type",
            "constants",
            "core-type",
            "core",
            "shared-type",
            "shared",
            "layout-type",
            "layout",
            "features-type",
            "features",
            ["type-parent", "type-sibling", "type-index"],
            ["parent", "sibling", "index"],
            "unknown",
          ],
          customGroups: [
            { groupName: "env-type", elementNamePattern: "^@env/", modifiers: ["type"] },
            { groupName: "env", elementNamePattern: "^@env/" },
            { groupName: "models-type", elementNamePattern: "^@models/", modifiers: ["type"] },
            { groupName: "models", elementNamePattern: "^@models/" },
            {
              groupName: "constants-type",
              elementNamePattern: "^@constants/",
              modifiers: ["type"],
            },
            { groupName: "constants", elementNamePattern: "^@constants/" },
            { groupName: "core-type", elementNamePattern: "^@core/", modifiers: ["type"] },
            { groupName: "core", elementNamePattern: "^@core/" },
            { groupName: "shared-type", elementNamePattern: "^@shared/", modifiers: ["type"] },
            { groupName: "shared", elementNamePattern: "^@shared/" },
            { groupName: "layout-type", elementNamePattern: "^@layout/", modifiers: ["type"] },
            { groupName: "layout", elementNamePattern: "^@layout/" },
            { groupName: "features-type", elementNamePattern: "^@features/", modifiers: ["type"] },
            { groupName: "features", elementNamePattern: "^@features/" },
          ],
        },
      ],
      "perfectionist/sort-named-imports": ["error", { type: "line-length", order: "asc" }],
      "@typescript-eslint/consistent-type-imports": "error",
    },
  },
  {
    files: ["src/main.ts"],
    rules: { "no-console": "off" },
  },
  {
    files: ["**/*.spec.ts"],
    rules: {
      "@typescript-eslint/no-empty-function": "off",
    },
  },
  {
    files: ["**/*.html"],
    extends: [...angular.configs.templateRecommended, ...angular.configs.templateAccessibility],
    rules: {
      "@angular-eslint/template/no-autofocus": "off",

      "@angular-eslint/template/eqeqeq": ["error", { allowNullOrUndefined: true }],
    },
  },
);
