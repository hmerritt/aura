#!/usr/bin/env bun
/* eslint-disable no-console */
import path from "path";
import { fileURLToPath } from "url";

import * as core from "./scripts/bootstrap/core";
import packageJSON from "./package.json";
import { type Env } from "./scripts/bootstrap/core";
import { adriftVersion, isAdriftUpdateAvailable } from "./scripts/bootstrap/version";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const pathRoot = __dirname;
const args = [...process.argv.slice(2)];

// Run bootrap
bootstrap();

// Bootstrap runs code before react start/build.
// Run anything you like, here we get the app version from the package.json + the current commit hash.
// prettier-ignore
async function bootstrap() {
	// Instruct Bun to use Node when flag is passed (required for compatibility)
	const useNode = args[0] === "--bun:node";
	if (useNode) args.shift();

	const isDev = core.isDev(args);
	const gitCommitHash = await core.run(`git rev-parse HEAD`, pathRoot, '');
	const gitCommitHashShort = core.shorten(gitCommitHash) || '';
	const gitBranch = await core.getGitBranch(pathRoot);
	const appVersion = packageJSON?.version;
	const appName = packageJSON?.name;

	// Checks GitHub for any adrift updates.
	const checkForAdriftUpdate = isDev;

	// When true, the env array below can be overridden by whatever is in the environment at runtime.
	const allowEnvOverride = true;

	// Set ENV array to inject, key/value
	const env: Env = [
		["NODE_ENV", core.getNodeEnv(args)],
		["GENERATE_SOURCEMAP", isDev],
		["VITE_ADRIFT_VERSION", adriftVersion],
		["VITE_NAME", appName],
		["VITE_VERSION", appVersion],
		["VITE_GIT_BRANCH", gitBranch],
		["VITE_GIT_COMMIT", gitCommitHashShort],
		["VITE_APP_HOST", "http://localhost:5173"],
		["VITE_SHOW_DEVTOOLS", true]
		// ['VITE_PLAUSIBLE_ENABLE', true],
		// ['VITE_PLAUSIBLE_DOMAIN', 'PLAUSIBLE_DOMAIN'],
		// ['VITE_PLAUSIBLE_API_HOST', 'https://plausible.io']
	];

	// Log app name and version info
	console.log(core.versionString(appName, appVersion, gitBranch, gitCommitHashShort), "\n");

	const update = await isAdriftUpdateAvailable();
	if (checkForAdriftUpdate && update) {
		console.log(`\x1b[33m`, `-> adrift update available! (${adriftVersion} - ${update})`, `\x1b[0m`, '\n');
	}

	// Run bootstrap script
	core.bootstrap(env, allowEnvOverride, args, useNode, pathRoot);
}
