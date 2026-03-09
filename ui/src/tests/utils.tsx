import {
	Outlet,
	createMemoryHistory,
	createRootRoute,
	createRoute,
	createRouter
} from "@tanstack/react-router";
import { render as reactRender } from "@testing-library/react";
import { afterAll, beforeAll } from "vitest";

import type { LogLevels } from "lib/global/log";
import { setGlobalValue } from "lib/global/utils";

/**
 * Create test router from element.
 *
 * @Note There is little to no documentation on best practices here.
 *
 * https://github.com/TanStack/router/discussions/604
 * https://github.com/TanStack/router/discussions/583
 * https://github.com/TanStack/router/discussions/198
 */
export const createTestRouter = (element: any, initialLocation = "/") => {
	const rootRoute = createRootRoute({
		component: Outlet
	});

	const componentRoute = createRoute({
		getParentRoute: () => rootRoute,
		path: "/",
		component: () => element
	});

	const router = createRouter({
		routeTree: rootRoute.addChildren([componentRoute]),
		history: createMemoryHistory({
			initialEntries: [initialLocation]
		}),
		context: {},
		defaultPendingMinMs: 0
	});

	return router;
};

/**
 * Shorthand for `document.querySelector`.
 *
 * `const { container } = render(<Home />);`
 */
export const select = (
	input: Element | ReturnType<typeof reactRender>,
	selectors: string
) => {
	const el = input instanceof Element ? input : input?.container;
	return el.querySelector(selectors);
};

/**
 * Wrapper for `select` that returns a `data-testid`.
 */
export const selectTestId = (
	input: Element | ReturnType<typeof reactRender>,
	testId: string
) => {
	const el = input instanceof Element ? input : input?.container;
	return el.querySelector(`[data-testid=${testId}]`);
};

/**
 * Shorthand for `window.getComputedStyle`.
 */
export const getStyle = (el: Element | null): CSSStyleDeclaration => {
	if (!el) return {} as CSSStyleDeclaration;
	return window.getComputedStyle(el);
};

/**
 * Standardize style string.
 */
export const cleanStyle = (s: string) => {
	s = s.replace(/\s+/g, " ").trim();
	return s;
};

/**
 * Helper for setting global log level
 */
export const setLogLevel = (level: LogLevels) => {
	setGlobalValue("logLevel", level, true);
};

/**
 * Silence logs during test runs
 */
export const silenceLogs = () => {
	beforeAll(() => {
		setLogLevel(0);
	});
	afterAll(() => {
		setLogLevel(4);
	});
};
