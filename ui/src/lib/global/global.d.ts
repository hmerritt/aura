import type { Scheduler } from "../scheduler";
import type { EnvKeys, EnvObj } from "./env";
import type { FeatureFn } from "./featureFlags";
import type { LogFn, LogLevels, LogStoreType, LognFn } from "./log";
import type { RunFn, RunSyncFn } from "./utils";

declare global {
	var __init: boolean;
	var debug: LogFn;
	var debugn: LognFn;
	var env: EnvObj;
	var envGet: (key: EnvKeys) => any;
	var feature: FeatureFn;
	var getNumberOfEventListeners: () => number;
	var getObjectOfEventListeners: () => Record<string, number>;
	var go: any;
	var log: LogFn;
	var logn: LognFn;
	var logLevel: LogLevels;
	var logStore: LogStoreType;
	var run: RunFn;
	var runSync: RunSyncFn;

	interface TaskController extends AbortController {
		setPriority(priority: string): void;
	}

	interface Window {
		__init: boolean;
		debug: LogFn;
		debugn: LognFn;
		env: EnvObj;
		envGet: (key: EnvKeys) => any;
		feature: FeatureFn;
		getNumberOfEventListeners: () => number;
		getObjectOfEventListeners: () => Record<string, number>;
		go: any;
		log: LogFn;
		logn: LognFn;
		logLevel: LogLevels;
		logStore: LogStoreType;
		run: RunFn;
		runSync: RunSyncFn;

		scheduler?: Scheduler;
		TaskController?: new (options?: { priority?: string }) => TaskController;
	}
}
