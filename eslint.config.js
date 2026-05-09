import js from "@eslint/js";
import globals from "globals";

export default [
  {
    ignores: ["**/node_modules/**", "src/styles/**", "art/**", "src/docs/**"],
  },
  {
    files: ["tests/**/*.js"],
    languageOptions: {
      globals: {
        ...globals.node,
        ...globals.browser,
        vi: "readonly",
        describe: "readonly",
        it: "readonly",
        expect: "readonly",
        beforeEach: "readonly",
        afterEach: "readonly",
      },
    },
  },
  {
    files: ["*.js", "*.mjs", "*.cjs"],
    languageOptions: {
      ecmaVersion: 2024,
      sourceType: "module",
      globals: {
        ...globals.node,
      },
    },
  },
  js.configs.recommended,
  {
    files: ["src/**/*.js", "src/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2024,
      sourceType: "module",
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      // Correctness — real bugs
      "no-console": ["error"],
      eqeqeq: ["error", "always", { null: "ignore" }],
      "no-var": "error",
      "prefer-const": ["error", { destructuring: "all" }],
      "no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrors: "all",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
      "no-implicit-globals": "error",
      "no-shadow-restricted-names": "error",
      "no-undef-init": "error",
      "no-self-compare": "error",
      "no-unmodified-loop-condition": "error",
      "no-template-curly-in-string": "error",
      "no-unreachable-loop": "error",
      "default-case-last": "error",
      "no-promise-executor-return": "error",
      "require-atomic-updates": "error",
      "no-async-promise-executor": "error",
      "no-constant-condition": ["error", { checkLoops: false }],
      curly: ["error", "all"],
      radix: "error",

      // Security / footguns
      "no-throw-literal": "error",
      "no-array-constructor": "error",
      "no-new-wrappers": "error",
      "no-eval": "error",
      "no-implied-eval": "error",
      "no-new-func": "error",
      "no-extend-native": "error",
      "no-extra-bind": "error",
      "no-floating-decimal": "error",
      "no-multi-str": "error",
      "no-octal-escape": "error",
      "no-proto": "error",
      "no-script-url": "error",
      "no-sequences": "error",
      "no-void": "error",
      yoda: "error",
      "prefer-template": "error",
      "prefer-arrow-callback": "error",
      "object-shorthand": "error",
      "no-useless-rename": "error",
      "no-useless-return": "error",
      "no-unused-expressions": ["error", { allowShortCircuit: true, allowTernary: true }],

      // Stylistic — relaxed for this codebase
      "no-shadow": "off",
      "no-await-in-loop": "off",
      "no-return-assign": ["error", "except-parens"],
    },
  },
];
