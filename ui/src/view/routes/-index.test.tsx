import { fireEvent, screen, waitFor } from "@testing-library/react";
import { readFileSync } from "fs";
import path from "path";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { env } from "lib/global/env";
import type { SettingsDocument, SettingsLoadResult } from "lib/host/types";

import { renderBasic } from "tests/render";

import { IndexRoute } from "./index";

const { loadSettingsMock } = vi.hoisted(() => ({
    loadSettingsMock: vi.fn()
}));

vi.mock("lib/host/client", () => ({
    auraSettingsHost: {
        request: loadSettingsMock
    }
}));

const cargoManifestPath = path.resolve(process.cwd(), "..", "Cargo.toml");
const cargoManifest = readFileSync(cargoManifestPath, "utf8");
const cargoVersion =
    cargoManifest
        .match(/^\[package\]([\s\S]*?)(?:^\[[^\]]+\]|\Z)/m)?.[1]
        ?.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m)?.[1] ?? "unknown";
const CURRENT_IMAGE_PREVIEW_SRC = "/preview/current?rev=current";
const NEXT_IMAGE_PREVIEW_SRC = "/preview/next?rev=next";
const PREVIEW_FRAME = {
    width: 18,
    height: 9
};

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
    previewFrame: PREVIEW_FRAME
});

beforeEach(() => {
    loadSettingsMock.mockReset();
    loadSettingsMock.mockResolvedValue(createSettingsLoadResult("image"));
});

describe("settings header", () => {
    test("renders the requested header layout", async () => {
        await renderBasic(<IndexRoute />);

        const header = screen.getByTestId("settings-header");
        const logo = screen.getByTestId("settings-header-logo");
        const expectedVersion = `Version ${cargoVersion}`;

        expect(env.appVersion).toBe(cargoVersion);
        expect(screen.getByText("aura")).toBeInTheDocument();
        expect(screen.getByText(expectedVersion)).toBeInTheDocument();
        expect(logo).toHaveAttribute("src", "/logo.png");
        expect(logo).toHaveAttribute("alt", "aura logo");
        expect(header).toHaveStyle("height: 55px");
        expect(header).toHaveStyle("width: 100%");
        expect(header).toHaveStyle("padding-left: 20px");
        expect(header).toHaveStyle("padding-right: 20px");
        expect(header).toHaveStyle("background-color: rgb(242, 242, 242)");
        expect(header).toHaveStyle("border-bottom-width: 1px");
        expect(header).toHaveStyle("border-bottom-style: solid");
        expect(header).toHaveStyle("border-bottom-color: rgb(218, 218, 218)");
    });

    test("loads the current renderer and shows both previews", async () => {
        await renderBasic(<IndexRoute />);

        const imageRadio = screen.getByRole("radio", { name: "Image" });
        const shaderRadio = screen.getByRole("radio", { name: "Shader" });

        await waitFor(() =>
            expect(loadSettingsMock).toHaveBeenCalledWith("load_settings", {})
        );
        await waitFor(() => expect(imageRadio).toBeChecked());

        expect(shaderRadio).not.toBeChecked();
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            CURRENT_IMAGE_PREVIEW_SRC
        );
        expect(screen.getByTestId("image-mode-preview-frame")).toHaveStyle(
            "aspect-ratio: 18 / 9"
        );
        expect(screen.getByTestId("image-mode-preview-frame")).toHaveStyle(
            "border-color: rgb(0, 183, 236)"
        );
        expect(screen.getByTestId("image-mode-preview-frame")).toHaveStyle(
            "box-shadow: 0 2px 10px rgba(0,0,0,.08)"
        );
        expect(screen.getByTestId("shader-mode-preview-frame")).toHaveStyle(
            "aspect-ratio: 18 / 9"
        );
        expect(screen.getByTestId("shader-mode-preview-frame")).toHaveStyle(
            "border-color: rgba(0, 0, 0, 0.12)"
        );
        expect(screen.getByTestId("shader-mode-preview").tagName.toLowerCase()).toBe(
            "canvas"
        );
    });

    test("reflects a shader document as the initial selection", async () => {
        loadSettingsMock.mockResolvedValueOnce(createSettingsLoadResult("shader"));

        await renderBasic(<IndexRoute />);

        const shaderRadio = screen.getByRole("radio", { name: "Shader" });
        await waitFor(() => expect(shaderRadio).toBeChecked());
        expect(screen.getByRole("radio", { name: "Image" })).not.toBeChecked();
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            NEXT_IMAGE_PREVIEW_SRC
        );
    });

    test("keeps the current image preview locked when shader is clicked", async () => {
        await renderBasic(<IndexRoute />);

        const imageRadio = screen.getByRole("radio", { name: "Image" });
        const shaderRadio = screen.getByRole("radio", { name: "Shader" });

        await waitFor(() => expect(imageRadio).toBeChecked());
        fireEvent.click(shaderRadio);

        expect(shaderRadio).toBeChecked();
        expect(imageRadio).not.toBeChecked();
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            CURRENT_IMAGE_PREVIEW_SRC
        );
    });

    test("locks the next preview when image is clicked from shader mode", async () => {
        loadSettingsMock.mockResolvedValueOnce(createSettingsLoadResult("shader"));

        await renderBasic(<IndexRoute />);

        const imageRadio = screen.getByRole("radio", { name: "Image" });
        const shaderRadio = screen.getByRole("radio", { name: "Shader" });

        await waitFor(() => expect(shaderRadio).toBeChecked());
        fireEvent.click(imageRadio);

        expect(imageRadio).toBeChecked();
        expect(shaderRadio).not.toBeChecked();
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            NEXT_IMAGE_PREVIEW_SRC
        );
        fireEvent.click(shaderRadio);
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            NEXT_IMAGE_PREVIEW_SRC
        );
    });

    test("renders no image preview when real previews are unavailable", async () => {
        loadSettingsMock.mockResolvedValueOnce({
            ...createSettingsLoadResult("image"),
            imagePreview: {
                currentId: null,
                currentSrc: null,
                nextId: null,
                nextSrc: null
            }
        });

        await renderBasic(<IndexRoute />);

        await waitFor(() =>
            expect(screen.getByRole("radio", { name: "Image" })).toBeChecked()
        );
        expect(screen.queryByTestId("image-mode-preview")).not.toBeInTheDocument();
        expect(screen.getByTestId("image-mode-preview-frame")).toHaveStyle(
            "aspect-ratio: 18 / 9"
        );
    });

    test("does not create a lock when shader mode has no real image preview", async () => {
        loadSettingsMock.mockResolvedValueOnce({
            ...createSettingsLoadResult("shader"),
            imagePreview: {
                currentId: "current",
                currentSrc: CURRENT_IMAGE_PREVIEW_SRC,
                nextId: null,
                nextSrc: null
            }
        });

        await renderBasic(<IndexRoute />);

        const imageRadio = screen.getByRole("radio", { name: "Image" });
        const shaderRadio = screen.getByRole("radio", { name: "Shader" });

        await waitFor(() => expect(shaderRadio).toBeChecked());
        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            CURRENT_IMAGE_PREVIEW_SRC
        );

        fireEvent.click(imageRadio);

        expect(screen.getByTestId("image-mode-preview")).toHaveAttribute(
            "src",
            CURRENT_IMAGE_PREVIEW_SRC
        );
    });
});
