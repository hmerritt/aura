import type { RendererMode, SettingsLoadResult } from "lib/host/types";

export type SettingsStatus = "idle" | "loading" | "loaded" | "error";

export type LockedImageSelection = {
    id: string;
    src: string;
};

export const DEFAULT_PREVIEW_FRAME = {
    width: 16,
    height: 9
} as const;

export function toLockedImageSelection(
    id: string | null | undefined,
    src: string | null | undefined
): LockedImageSelection | null {
    if (!id || !src) return null;
    return { id, src };
}

export function resolveLockedImageSelectionOnRendererChange(
    result: SettingsLoadResult,
    lockedImageSelection: LockedImageSelection | null,
    renderer: RendererMode
): LockedImageSelection | null {
    if (renderer !== "image" || lockedImageSelection !== null) {
        return lockedImageSelection;
    }

    const currentImageSelection = toLockedImageSelection(
        result.imagePreview.currentId,
        result.imagePreview.currentSrc
    );
    const nextImageSelection = toLockedImageSelection(
        result.imagePreview.nextId,
        result.imagePreview.nextSrc
    );

    return result.document.renderer === "image"
        ? currentImageSelection ?? nextImageSelection
        : nextImageSelection ?? currentImageSelection;
}

export function resolveImageModePreviewSrc(
    result: SettingsLoadResult | null,
    lockedImageSelection: LockedImageSelection | null
): string | null {
    if (!result) {
        return null;
    }

    const currentImageSelection = toLockedImageSelection(
        result.imagePreview.currentId,
        result.imagePreview.currentSrc
    );
    const nextImageSelection = toLockedImageSelection(
        result.imagePreview.nextId,
        result.imagePreview.nextSrc
    );
    const derivedImageSelection =
        result.document.renderer === "image"
            ? currentImageSelection ?? nextImageSelection
            : nextImageSelection ?? currentImageSelection;

    return (lockedImageSelection ?? derivedImageSelection)?.src ?? null;
}
