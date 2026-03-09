import { DeepKeyofPaths } from "lib/type-assertions";
import { get } from "lib/type-guards";

import { parseEnv, setGlobalValue } from "./utils";

/**
 * Environment variables.
 *
 * Add all environment variables here to ensure type safety.
 */
export const env = Object.freeze({
    // Core
    appName: "aura", // Optionally use `import.meta.env.VITE_NAME`
    appVersion: import.meta.env.VITE_VERSION,
    gitBranch: import.meta.env.VITE_GIT_BRANCH,
    gitCommitHash: import.meta.env.VITE_GIT_COMMIT,
    adriftVersion: import.meta.env.VITE_ADRIFT_VERSION,
    showDevTools:
        import.meta.env.MODE === "development" &&
        parseEnv(import.meta.env.VITE_SHOW_DEVTOOLS),
    plausible: {
        enable: parseEnv(import.meta.env.VITE_PLAUSIBLE_ENABLE),
        domain: import.meta.env.VITE_PLAUSIBLE_DOMAIN,
        apiHost: import.meta.env.VITE_PLAUSIBLE_API_HOST
    },
    mode: import.meta.env.MODE,
    isDev: import.meta.env.MODE === "development",
    isProd: import.meta.env.MODE === "production",
    isTest: import.meta.env.MODE === "test",
    isStage: import.meta.env.MODE === "stage" || import.meta.env.MODE === "staging",
    // Features
    timerIncrement: parseEnv(import.meta.env.VITE_FEATURE_INCREMENT),
    someOtherFeature: false
});

/**
 * Resolve value from env object.
 *
 * Supports resolving values nested in objects.
 *
 * @example envGet("plausible.enable") -> true
 */
export const envGet = (key: EnvKeys) => {
    return get(env, key);
};

export type EnvObj = typeof env;
export type EnvKeys = DeepKeyofPaths<EnvObj>;

export const injectEnv = () => {
    setGlobalValue("env", env);
    setGlobalValue("envGet", envGet);
};
