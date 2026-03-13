import { beforeEach, describe, expect, test, vi } from "vitest";

import type { SettingsDocument, SettingsLoadResult } from "lib/host/types";
import { store } from "state";
import {
    settingsLoad,
    settingsReset,
    settingsSetRenderer
} from "state/slices/settings/settingsActions";
import { colorStore } from "../color/colorStore";
import { countStore } from "../count/countStore";
import { settingsStore } from "./settingsStore";

const { loadSettingsMock } = vi.hoisted(() => ({
    loadSettingsMock: vi.fn()
}));

vi.mock("lib/host/client", () => ({
    auraSettingsHost: {
        request: loadSettingsMock
    }
}));

const CURRENT_IMAGE_PREVIEW_SRC = "/preview/current?rev=current";
const NEXT_IMAGE_PREVIEW_SRC = "/preview/next?rev=next";

const createSettingsDocument = (
    renderer: SettingsDocument["renderer"]
): SettingsDocument => ({
    renderer,
    image: {
        timer: "30m",
        remoteUpdateTimer: "4h",
        sources: [
            {
                type: "directory",
                path: "C:/Users/you/Pictures",
                recursive: true,
                extensions: ["jpg", "png"]
            }
        ],
        format: "jpg",
        jpeg_quality: 90
    },
    shader: {
        name: "gradient_glossy",
        target_fps: 60,
        resolution: 100,
        mouse_enabled: false,
        desktop_scope: "virtual",
        color_space: "unorm"
    },
    updater: {
        enabled: true,
        checkInterval: "6h",
        feedUrl: "https://github.com/hmerritt/aura/releases/latest/download"
    },
    cache_dir: "C:/Users/you/AppData/Local/aura/cache",
    state_file: "C:/Users/you/AppData/Local/aura/state.json",
    log_level: "info",
    max_cache_mb: 1024,
    max_cache_age_days: 30
});

const createSettingsLoadResult = (
    renderer: SettingsDocument["renderer"]
): SettingsLoadResult => ({
    document: createSettingsDocument(renderer),
    warnings: [],
    imagePreview: {
        currentId: "current",
        currentSrc: CURRENT_IMAGE_PREVIEW_SRC,
        nextId: "next",
        nextSrc: NEXT_IMAGE_PREVIEW_SRC
    },
    previewFrame: {
        width: 18,
        height: 9
    }
});

beforeEach(() => {
    store.setState(() => ({
        color: { ...colorStore, colors: [...colorStore.colors] },
        count: { ...countStore },
        settings: { ...settingsStore }
    }));
    settingsReset();
    loadSettingsMock.mockReset();
    loadSettingsMock.mockResolvedValue(createSettingsLoadResult("image"));
});

describe("settingsLoad", () => {
    test("transitions from idle to loaded and stores the result", async () => {
        const pendingResult = createSettingsLoadResult("image");
        let resolveLoad!: (value: SettingsLoadResult) => void;

        loadSettingsMock.mockReturnValueOnce(
            new Promise<SettingsLoadResult>((resolve) => {
                resolveLoad = resolve;
            })
        );

        const loadPromise = settingsLoad();

        expect(store.state.settings.status).toBe("loading");

        resolveLoad(pendingResult);
        await loadPromise;

        expect(store.state.settings.status).toBe("loaded");
        expect(store.state.settings.result).toEqual(pendingResult);
        expect(store.state.settings.lockedImageSelection).toEqual({
            id: "current",
            src: CURRENT_IMAGE_PREVIEW_SRC
        });
    });

    test("sets error when the host request fails", async () => {
        loadSettingsMock.mockRejectedValueOnce(new Error("boom"));

        await settingsLoad();

        expect(store.state.settings.status).toBe("error");
        expect(store.state.settings.result).toBeNull();
    });
});

describe("settingsSetRenderer", () => {
    test("locks the next preview when switching to image from shader mode", async () => {
        loadSettingsMock.mockResolvedValueOnce(createSettingsLoadResult("shader"));

        await settingsLoad();
        settingsSetRenderer("image");

        expect(store.state.settings.result?.document.renderer).toBe("image");
        expect(store.state.settings.lockedImageSelection).toEqual({
            id: "next",
            src: NEXT_IMAGE_PREVIEW_SRC
        });
    });

    test("preserves the current lock when switching away from image mode", async () => {
        await settingsLoad();
        settingsSetRenderer("shader");

        expect(store.state.settings.result?.document.renderer).toBe("shader");
        expect(store.state.settings.lockedImageSelection).toEqual({
            id: "current",
            src: CURRENT_IMAGE_PREVIEW_SRC
        });
    });
});
