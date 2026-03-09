import { fixupPluginRules } from "@eslint/compat";
import stylex from "@stylexjs/eslint-plugin";
import typescript from "@typescript-eslint/eslint-plugin";
import typescriptParser from "@typescript-eslint/parser";
import reactCompiler from "eslint-plugin-react-compiler";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

const ignores = [
	".expo-shared",
	".expo",
	".git",
	".github",
	".husky",
	".idea",
	".next",
	".tanstack",
	".turbo",
	".vscode",
	".yarn",
	"build/*",
	"coverage/*",
	"dist/*",
	"node_modules/*",
	"src/routeTree.gen.ts",
	"src/lib/device.ts"
];

export default [
	{
		ignores
	},
	{
		files: ["src/**/*.{ts,tsx}"],
		languageOptions: {
			parser: typescriptParser,
			parserOptions: {
				ecmaVersion: "latest",
				sourceType: "module",
				ecmaFeatures: {
					jsx: true
				}
			},
			globals: {
				...globals.browser,
				document: true,
				window: true
			}
		},
		plugins: {
			"@stylexjs": stylex,
			"@typescript-eslint": typescript,
			"react-compiler": fixupPluginRules(reactCompiler),
			"react-hooks": fixupPluginRules(reactHooks)
		},
		rules: {
			"@typescript-eslint/no-non-null-assertion": "warn",
			"react-hooks/rules-of-hooks": "error",
			"react-hooks/exhaustive-deps": "warn",
			"react-compiler/react-compiler": "error",
			"@stylexjs/valid-styles": "warn",
			"@stylexjs/no-unused": "warn",
			"@stylexjs/valid-shorthands": "warn",
			"@stylexjs/sort-keys": "off",
			"no-console": ["warn", { allow: ["warn", "error"] }]
		}
	}
];
