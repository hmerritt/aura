import { beforeEach, describe, expect, test } from "vitest";

import { normalizeAuraSettingsEntryUrl, syncAuraSettingsEntryPath } from "./router";

describe("router bootstrap", () => {
    beforeEach(() => {
        window.history.replaceState(null, "", "/");
    });

    test("normalizes /index.html to root", () => {
        expect(normalizeAuraSettingsEntryUrl("/index.html")).toBe("/");
    });

    test("preserves search and hash when normalizing /index.html", () => {
        expect(
            normalizeAuraSettingsEntryUrl("/index.html", "?tab=general", "#advanced")
        ).toBe("/?tab=general#advanced");
    });

    test("does not rewrite non-entry paths", () => {
        expect(normalizeAuraSettingsEntryUrl("/")).toBeNull();
        expect(normalizeAuraSettingsEntryUrl("/settings")).toBeNull();
        expect(normalizeAuraSettingsEntryUrl("/assets/index.js")).toBeNull();
    });

    test("rewrites the browser location before router matching", () => {
        window.history.replaceState(null, "", "/index.html?tab=general#advanced");

        syncAuraSettingsEntryPath();

        expect(window.location.pathname).toBe("/");
        expect(window.location.search).toBe("?tab=general");
        expect(window.location.hash).toBe("#advanced");
    });
});
