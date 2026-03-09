import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

import { env } from "lib/global/env";
import { injectLog } from "lib/global/log";
import { injectRun, setGlobalValue } from "lib/global/utils";

import "../lib/styles/stylex.css";

setGlobalValue("env", env, true);
injectRun();
injectLog();
global.envGet = vi.fn();
global.feature = vi.fn();

HTMLCanvasElement.prototype.getContext = vi.fn();

afterEach(() => {
	cleanup();
});
