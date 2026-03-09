import { env } from "./env";

/**
 * Returns version string including app name, version, git branch, and commit hash.
 *
 * E.g `App [Version 1.0.0 (development 4122b6...dc7c)]`
 */
export const versionString = () => {
	if (!env.appVersion) {
		return `${env.appName} [Version unknown]`;
	}

	let versionString = `${env.appName} [Version ${env.appVersion}`;

	if (env.gitCommitHash) {
		versionString += ` (`;

		// ENV (hide in production)
		if (!env.isProd) {
			versionString += `${env.mode || "unknown"} `;
		}

		// Branch name (hide in production)
		if (!env.isProd && env.gitBranch !== "master") {
			versionString += `${env.gitBranch || "unknown"}/`;
		}

		// Commit hash
		versionString += `${env.gitCommitHash})`;
	}

	versionString += `]`;

	return versionString;
};
