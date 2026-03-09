import {
	Outlet,
	createRootRouteWithContext,
	useRouterState
} from "@tanstack/react-router";
import { lazy } from "react";

import { Icon } from "view/components";

/**
 * `@tanstack/react-router` file-based routing.
 *
 * https://tanstack.com/router/latest/docs/framework/react/overview
 */
export const Route = createRootRouteWithContext()({
	component: RootRoute
});

function RootRoute() {
	return (
		<>
			{/* Show a global spinner when the router is transitioning */}
			<RouterSpinner />
			{/* Render our first route match */}
			<Outlet />
			{/* Router dev tools */}
			<TanStackRouterDevtools />
		</>
	);
}

const TanStackRouterDevtools = feature("showDevTools", { alwaysShowOnDev: false })
	? lazy(() =>
		import("@tanstack/react-router-devtools").then((res) => ({
			default: res.TanStackRouterDevtools
		}))
	)
	: () => null;

function RouterSpinner() {
	const isLoading = useRouterState({ select: (s) => s.isLoading });
	return isLoading ? <Icon name="Spinner" /> : null;
}
