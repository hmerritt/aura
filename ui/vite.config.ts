import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { ViteMinifyPlugin as minify } from "vite-plugin-minify";
import tsconfigPaths from "vite-tsconfig-paths";
import { ViteUserConfig, defineConfig } from "vitest/config";

import styleX from "./config/stylex";

const isProd = process.env.NODE_ENV === "production";
const isDev = !isProd;
const isTest = process.env.NODE_ENV === "test";

const aliases = {
    "lib/*": [path.join(__dirname, "src/lib/*")],
    "state/*": [path.join(__dirname, "src/state/*")],
    "tests/*": [path.join(__dirname, "src/tests/*")],
    "types/*": [path.join(__dirname, "src/types/*")],
    "view/*": [path.join(__dirname, "src/view/*")]
};
const exclude = [
    ".cache",
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
    "build",
    "coverage",
    "dist",
    "node_modules",
    "tests-e2e"
];

// https://vitejs.dev/config/
export default defineConfig({
    build: {
        sourcemap: isDev,
        minify: isProd,
        target: "es2022",
        outDir: "dist",
        emptyOutDir: true,
        rollupOptions: {
            // Bug in react-router-devtools? - this is required now:
            external: ["solid-js", "solid-js/web"]
        }
    },
    define: {
        "process.env": {}
    },
    css: {
        preprocessorOptions: {
            scss: {
                api: "modern-compiler"
            }
        }
    },
    plugins: [
        tsconfigPaths(),
        react({
            babel: {
                plugins: ["babel-plugin-react-compiler"]
            }
        }),
        styleX({
            aliases,
            debug: isDev,
            test: false, // Breaks CSS injection for some reason
            runtimeInjection: isTest,
            useCSSLayers: true
        }),
        tanstackRouter({
            routesDirectory: "src/view/routes"
        }),
        minify({
            minifyCSS: true
        })
    ],
    test: {
        globals: false,
        environment: "jsdom",
        setupFiles: "./src/tests/setupTests.ts",
        css: true, // @Note Parsing CSS is slow
        exclude: exclude,
        coverage: {
            enabled: false,
            provider: "v8"
        },
        benchmark: {
            include: ["**/*.{bench,benchmark}.?(c|m)[jt]s?(x)"],
            exclude: exclude
        },
        // Debug
        logHeapUsage: true
    }
} as ViteUserConfig);
