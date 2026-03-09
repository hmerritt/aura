import { type EnvKeys, envGet } from "./env";
import { parseEnv, setGlobalValue } from "./utils";

/**
 * Returns `true` if the feature is enabled in `env` object.
 *
 * `true` being any non-falsy value, plus string versions of falsy values such as `"false"`, `"null"`, ect...
 */
export const feature = (mode: FeatureFlags, options: FeatureOptions = {}): boolean => {
	const { alwaysShowOnDev } = {
		alwaysShowOnDev: true,
		...options
	};

	// Bypass feature flag in dev mode if `alwaysShowOnDev` is true (unless explicitly set to false)
	if (
		alwaysShowOnDev &&
		(envGet("isDev") || envGet("isTest")) &&
		parseEnv(envGet(mode)) !== false
	) {
		return true;
	}

	// Feature is truthy in featureFlags{}
	if (envGet(mode) && parseEnv(envGet(mode))) {
		return true;
	}

	return false;
};

export type FeatureOptions = {
	alwaysShowOnDev?: boolean;
};

export type FeatureFlags = EnvKeys;
export type FeatureFn = typeof feature;

export const injectFeature = () => {
	setGlobalValue("feature", feature);
};
