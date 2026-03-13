import { ErrorComponent, createRouter } from "@tanstack/react-router";

import { routeTree } from "./routeTree.gen";

const INDEX_ENTRY_PATH = "/index.html";

export const normalizeAuraSettingsEntryUrl = (
    pathname: string,
    search = "",
    hash = ""
) => {
    if (pathname !== INDEX_ENTRY_PATH) {
        return null;
    }

    return `/${search}${hash}`;
};

export const syncAuraSettingsEntryPath = (
    win: Pick<Window, "history" | "location"> = window
) => {
    const normalizedUrl = normalizeAuraSettingsEntryUrl(
        win.location.pathname,
        win.location.search,
        win.location.hash
    );

    if (normalizedUrl) {
        win.history.replaceState(win.history.state, "", normalizedUrl);
    }
};

const createAppRouter = () => {
    syncAuraSettingsEntryPath();

    return createRouter({
        routeTree,
        scrollRestoration: true,
        scrollRestorationBehavior: "instant",
        defaultPreload: "intent",
        defaultPendingComponent: () => null,
        defaultErrorComponent: ({ error }: { error: Error }) => (
            <ErrorComponent error={error} />
        )
    });
};

export const router = createAppRouter();

declare module "@tanstack/react-router" {
    interface Register {
        router: typeof router;
    }
}
