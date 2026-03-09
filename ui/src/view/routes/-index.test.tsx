import { screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { renderBasic } from "tests/render";

import { IndexRoute } from "./index";

describe("settings header", () => {
	test("renders the requested header layout", async () => {
		await renderBasic(<IndexRoute />);

		const header = screen.getByTestId("settings-header");
		const logo = screen.getByTestId("settings-header-logo");

		expect(screen.getByText("aura")).toBeInTheDocument();
		expect(screen.getByText("Version 0.22.32")).toBeInTheDocument();
		expect(logo).toHaveAttribute("src", "/logo.png");
		expect(logo).toHaveAttribute("alt", "aura logo");
		expect(header).toHaveStyle("height: 55px");
		expect(header).toHaveStyle("width: 100%");
		expect(header).toHaveStyle("padding: 15px 24px");
		expect(header).toHaveStyle("background-color: rgb(242, 242, 242)");
		expect(header).toHaveStyle("border-bottom-width: 1px");
		expect(header).toHaveStyle("border-bottom-style: solid");
		expect(header).toHaveStyle("border-bottom-color: rgb(218, 218, 218)");
	});
});
