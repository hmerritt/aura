import { auraSettingsHost } from "lib/host/client";
import type { RendererMode } from "lib/host/types";
import { updateSlice } from "state/index";

import {
    resolveLockedImageSelectionOnRendererChange,
    toLockedImageSelection
} from "./settingsShared";

export const settingsLoad = async () => {
    let shouldLoad = false;

    updateSlice("settings", (settings) => {
        if (settings.status === "loading") {
            return;
        }

        settings.status = "loading";
        shouldLoad = true;
    });

    if (!shouldLoad) {
        return;
    }

    try {
        const result = await auraSettingsHost.request("load_settings", {});

        updateSlice("settings", (settings) => {
            settings.status = "loaded";
            settings.result = result;
            settings.lockedImageSelection =
                result.document.renderer === "image"
                    ? toLockedImageSelection(
                          result.imagePreview.currentId,
                          result.imagePreview.currentSrc
                      )
                    : null;
        });
    } catch {
        updateSlice("settings", (settings) => {
            settings.status = "error";
        });
    }
};

export const settingsSetRenderer = (renderer: RendererMode) => {
    updateSlice("settings", (settings) => {
        if (!settings.result) {
            return;
        }

        settings.lockedImageSelection = resolveLockedImageSelectionOnRendererChange(
            settings.result,
            settings.lockedImageSelection,
            renderer
        );
        settings.result.document.renderer = renderer;
    });
};

export const settingsReset = () => {
    updateSlice("settings", (settings) => {
        settings.status = "idle";
        settings.result = null;
        settings.lockedImageSelection = null;
    });
};
