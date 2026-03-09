/* eslint-disable no-console */
import path from "node:path";

import * as core from "./core";

/**
 * Internal adrift version.
 */
export const adriftVersion = "0.14.836";

/**
 * Bumps the adrift `patch` version number using the total commit count.
 *
 * Directly changes the above variable `adriftVersion` (by overwriting this file).
 */
export async function bumpAdriftPatchVersion() {
	try {
		const __dirname = import.meta.dir || ".";
		const pathRoot = path.dirname(path.dirname(__dirname));

		// Get the total commit count
		const commitCount = (
			await core.run(`git rev-list --count HEAD`, pathRoot, "")
		).trim();

		// Read the contents of version.ts
		const versionFile = path.join(__dirname, "version.ts");
		const versionFileContent = await Bun.file(versionFile).text();

		// Extract the version number parts
		const versionMatch = versionFileContent.match(
			/(const adriftVersion = ")(\d+\.\d+\.)(\d+)(")/
		);
		const majorMinor = versionMatch?.[2];
		const newVersion = `${majorMinor}${commitCount}`;

		if (!majorMinor) {
			throw new Error("No version number found in version.ts");
		}

		// Replace the version patch with the commit count
		const updatedContent = versionFileContent.replace(
			/(const adriftVersion = ")(\d+\.\d+\.)\d+(")/g,
			`$1${newVersion}$3`
		);

		// Write the updated content back to version.ts
		Bun.write(versionFile, updatedContent);

		console.log(`\x1b[36madrift@${newVersion}\x1b[0m`);
	} catch (error) {
		console.error("\x1b[31mError bumping adrift patch version:", error, `\x1b[0m`);
	}
}

/**
 * Checks with latest GitHub release to see if there is an update.
 *
 * @returns latest version number
 */
export async function isAdriftUpdateAvailable() {
	try {
		const url =
			"https://raw.githubusercontent.com/hmerritt/adrift/master/scripts/bootstrap/version.ts";
		const rawGithubText = await (await fetch(url)).text();

		const versionRegex = /adriftVersion\s*=\s*"([^"]+)"/;
		const match = rawGithubText?.match(versionRegex)?.[1]?.trim();

		if (!match || !match?.match(/\d+\.\d+\.\d+/gi)) {
			throw new Error("No version found");
		}

		// Compare versions
		const current = adriftVersion.split(".").map((x) => Number(x));
		const latest = match.split(".").map((x) => Number(x));
		let comparison = 0;
		for (let i = 0; i < Math.max(current.length, latest.length); i++) {
			if ((current[i] || 0) < (latest[i] || 0)) {
				comparison = -1;
				break;
			} else if ((current[i] || 0) > (latest[i] || 0)) {
				comparison = 1;
			}
		}

		if (comparison === -1) {
			return match;
		}
	} catch (_) {
		// Swallow error
	}

	return false;
}
