import { readFileSync } from "fs";
import path from "path";

import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, beforeEach, vi } from "vitest";

import { env } from "lib/global/env";
import type { SettingsDocument } from "lib/host/types";
import { renderBasic } from "tests/render";

const { loadSettingsMock } = vi.hoisted(() => ({
	loadSettingsMock: vi.fn()
}));

vi.mock("lib/host/client", () => ({
	auraSettingsHost: {
		request: loadSettingsMock
	}
}));

import { IndexRoute } from "./index";

const cargoManifestPath = path.resolve(process.cwd(), "..", "Cargo.toml");
const cargoManifest = readFileSync(cargoManifestPath, "utf8");
const cargoVersion =
	cargoManifest.match(/^\[package\]([\s\S]*?)(?:^\[[^\]]+\]|\Z)/m)?.[1]?.match(
		/^\s*version\s*=\s*"([^"]+)"\s*$/m
	)?.[1] ?? "unknown";

const createSettingsDocument = (renderer: SettingsDocument["renderer"]): SettingsDocument => ({
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

beforeEach(() => {
	loadSettingsMock.mockReset();
	loadSettingsMock.mockResolvedValue({
		document: createSettingsDocument("image"),
		warnings: []
	});
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
		expect(header).toHaveStyle("padding-top: 15px");
		expect(header).toHaveStyle("padding-bottom: 15px");
		expect(header).toHaveStyle("padding-left: 24px");
		expect(header).toHaveStyle("padding-right: 24px");
		expect(header).toHaveStyle("background-color: rgb(242, 242, 242)");
		expect(header).toHaveStyle("border-bottom-width: 1px");
		expect(header).toHaveStyle("border-bottom-style: solid");
		expect(header).toHaveStyle("border-bottom-color: rgb(218, 218, 218)");
	});

	test("loads the current renderer and shows both previews", async () => {
		await renderBasic(<IndexRoute />);

		const imageRadio = screen.getByRole("radio", { name: "Image" });
		const shaderRadio = screen.getByRole("radio", { name: "Shader" });

		await waitFor(() => expect(loadSettingsMock).toHaveBeenCalledWith("load_settings", {}));
		await waitFor(() => expect(imageRadio).toBeChecked());

		expect(shaderRadio).not.toBeChecked();
		expect(screen.getByTestId("image-mode-preview")).toHaveAttribute("src", expect.stringContaining("data:image/svg+xml"));
		expect(screen.getByTestId("shader-mode-preview").tagName.toLowerCase()).toBe("canvas");
	});

	test("reflects a shader document as the initial selection", async () => {
		loadSettingsMock.mockResolvedValueOnce({
			document: createSettingsDocument("shader"),
			warnings: []
		});

		await renderBasic(<IndexRoute />);

		const shaderRadio = screen.getByRole("radio", { name: "Shader" });
		await waitFor(() => expect(shaderRadio).toBeChecked());
		expect(screen.getByRole("radio", { name: "Image" })).not.toBeChecked();
	});

	test("updates the selected mode when shader is clicked", async () => {
		await renderBasic(<IndexRoute />);

		const imageRadio = screen.getByRole("radio", { name: "Image" });
		const shaderRadio = screen.getByRole("radio", { name: "Shader" });

		await waitFor(() => expect(imageRadio).toBeChecked());
		fireEvent.click(shaderRadio);

		expect(shaderRadio).toBeChecked();
		expect(imageRadio).not.toBeChecked();
	});

	test("updates the selected mode when image is clicked", async () => {
		loadSettingsMock.mockResolvedValueOnce({
			document: createSettingsDocument("shader"),
			warnings: []
		});

		await renderBasic(<IndexRoute />);

		const imageRadio = screen.getByRole("radio", { name: "Image" });
		const shaderRadio = screen.getByRole("radio", { name: "Shader" });

		await waitFor(() => expect(shaderRadio).toBeChecked());
		fireEvent.click(imageRadio);

		expect(imageRadio).toBeChecked();
		expect(shaderRadio).not.toBeChecked();
	});
});
