/* eslint-disable no-console */
import { adriftVersion } from "./version";

export type Env = [string, any][];

const __dirname = import.meta.dir || ".";
const PROD_COMMANDS = new Set(["build", "preview"]);
const TEST_COMMANDS = new Set(["vitest", "cosmos"]);

const getBunRuntime = () => {
	if (!globalThis.Bun) {
		throw new Error("Bun runtime is required to run bootstrap scripts.");
	}

	return globalThis.Bun;
};

const getShellCommand = (command: string) => {
	if (process.platform === "win32") {
		return ["cmd.exe", "/d", "/s", "/c", command];
	}

	return ["/bin/sh", "-lc", command];
};

/**
 * Bootstrap runs code before react start/build.
 *
 * Injects ENV array into cross-env before running script
 */
export async function bootstrap(
	env: Env,
	allowEnvOverride: boolean | undefined,
	args = [] as string[],
	useNode: boolean,
	path: string | undefined
) {
	try {
		// Build ENV + Arguments string
		const envArr = allowEnvOverride ? overrideHardcodedENV(env) : env;
		const envString = buildENV(envArr);
		const argString = args?.join(" ") || "";
		const bunCommand = useNode ? "bunx" : "bunx --bun";

		// Run scripts/start|build command
		runStream(`${bunCommand} cross-env ${envString} ${argString}`, path);
	} catch (error) {
		console.error("[bootstrap]", error);
	}
}

/**
 * Shortens a string at both ends, separated by `...`, eg `123456789` -> `12345...789`
 */
export function shorten(str: string | undefined, numCharsStart = 6, numCharsEnd = 4) {
	if (!str || str?.length <= 11) return str;
	return `${str.substring(0, numCharsStart)}...${str.slice(str.length - numCharsEnd)}`;
}

/**
 * Returns the current git branch
 */
export async function getGitBranch(path: string, fallback = undefined) {
	let gitBranch = await run(`git rev-parse --abbrev-ref HEAD`, path, null);

	// Detect CircleCI
	if (process.env.CIRCLE_BRANCH) {
		gitBranch = process.env.CIRCLE_BRANCH;
	}

	// Detect GitHub Actions CI
	if (process.env.GITHUB_REF_NAME && process.env.GITHUB_REF_TYPE === "branch") {
		gitBranch = process.env.GITHUB_REF_NAME;
	}

	// Detect GitLab CI
	if (process.env.CI_COMMIT_BRANCH) {
		gitBranch = process.env.CI_COMMIT_BRANCH;
	}

	// Detect Netlify CI + generic
	if (process.env.BRANCH) {
		gitBranch = process.env.BRANCH;
	}

	// Detect HEAD state, and remove it.
	if (gitBranch === "HEAD") {
		gitBranch = fallback;
	}

	return gitBranch;
}

/**
 * Use ENV values in current environment over hardcoded values
 */
export function overrideHardcodedENV(env = [] as Env) {
	let overrideCount = 0;

	const newEnv: Env = env.map((envItem) => {
		const [name, value] = envItem;
		const envValue = process.env[name];

		if (envValue && envValue !== `${value}`) {
			console.warn(
				`WARN: Overriding hardcoded ${name} value: ${value} => ${envValue}`
			);
			overrideCount++;
			return [name, envValue];
		}

		return envItem;
	});

	if (overrideCount > 0) console.warn("");

	return newEnv;
}

/**
 * Handles ENV array and build a string to use
 */
export function buildENV(env = [] as Env) {
	if (env.length < 1) return "";

	console.log("Building ENV to inject:");

	// Build ENV string
	let envString = "";
	env.forEach((item, index) => {
		if (index > 0) envString += ` `;
		const envPair = `${item[0]}=${item[1]}`;
		envString += envPair;
		console.log("  ", index + 1, envPair);
	});

	console.log("");

	return envString;
}

/**
 * Execute OS commands, awaits response from stdout
 */
export async function run(
	command: string,
	path = __dirname,
	fallback = undefined as any
) {
	try {
		const bunRuntime = getBunRuntime();
		const execProcess = bunRuntime.spawn({
			cmd: getShellCommand(command),
			cwd: path,
			stdout: "pipe",
			stderr: "pipe"
		});

		const [code, stdout, stderr] = await Promise.all([
			execProcess.exited,
			execProcess.stdout
				? new Response(execProcess.stdout).text()
				: Promise.resolve(""),
			execProcess.stderr
				? new Response(execProcess.stderr).text()
				: Promise.resolve("")
		]);

		if (code !== 0) {
			throw new Error(stderr || `Command failed with exit code ${code}`);
		}

		return stdout?.trim();
	} catch (e) {
		if (fallback === undefined) {
			// Should contain code (exit code) and signal (that caused the termination).
			console.error("[run]", e);
		} else {
			console.log("[run] (using fallback)", e);
			return fallback;
		}
	}
}

/**
 * Execute OS commands, streams response from stdout
 */
export function runStream(command: string, path = __dirname, exitOnError = true) {
	const bunRuntime = getBunRuntime();
	const execProcess = bunRuntime.spawn({
		cmd: getShellCommand(command),
		cwd: path,
		stdout: "inherit",
		stderr: "inherit"
	});

	void execProcess.exited.then((code: number) => {
		if (code !== 0) {
			console.log("ERROR: process finished with a non-zero code");
			if (exitOnError) process.exit(1);
		}
	});
}

export function isProd(args = [] as string[]) {
	const command = args[1];
	return !!command && PROD_COMMANDS.has(command);
}

export function isTest(args = [] as string[]) {
	const command = args[0];
	return !!command && TEST_COMMANDS.has(command);
}

export function isDev(args = [] as string[]) {
	return !isProd(args) && !isTest(args);
}

/**
 * Determine `NODE_ENV` from args passed to the script.
 *
 * @returns `NODE_ENV` value
 */
export function getNodeEnv(args = [] as string[]) {
	if (isProd(args)) return "production";
	if (isTest(args)) return "test";
	return "development";
}

/**
 * Returns version string including app name, version, git branch, and commit hash.
 *
 * This has been refactored from `/src/lib/global/version.ts`. @TODO make shared function and remove this one.
 *
 * E.g `App [Version 1.0.0 (development 4122b6...dc7c)]`
 */
export const versionString = (
	appName = undefined as string | undefined,
	appVersion = undefined as string | undefined,
	gitBranch = undefined as string | undefined,
	gitCommitHash = undefined as string | undefined
) => {
	if (!appVersion) {
		return `${appName} [Version unknown]`;
	}

	let versionString = `${appName} [Version ${appVersion}`;

	if (gitCommitHash) {
		versionString += ` (`;

		// Branch name
		versionString += `${gitBranch || "unknown"}/`;

		// Commit hash
		versionString += `${gitCommitHash})`;
	}

	versionString += `]`;

	if (adriftVersion) versionString += ` with \x1b[36madrift@${adriftVersion}\x1b[0m`;

	return versionString;
};
