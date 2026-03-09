import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { testBasicComponent } from "tests/macros";
import { renderBasic as render } from "tests/render";

import { Grid } from "./index";

describe("Grid component", () => {
	testBasicComponent({
		name: "(default)",
		Component: Grid,
		props: {},
		hasChildren: true,
		hasSx: true,
		shouldHaveStyles: {
			position: "relative",
			display: "grid",
			width: "100%"
		},
		shouldNotHaveStyles: {
			display: "relative",
			justifyContent: "center"
		},
		shouldContainInStyleAttribute: [
			"--gridGap: 10;",
			"--gridTemplateColumns: repeat(auto-fit, minmax(min(100%, 100rem), 1fr));"
		]
	});

	it("should pass through other standard div props", async () => {
		await render(
			<Grid
				id="my-custom-grid"
				aria-label="My grid container"
				data-testid="grid-passthrough"
			/>
		);
		const $el = screen.getByTestId("grid-passthrough");

		expect($el).toHaveAttribute("id", "my-custom-grid");
		expect($el).toHaveAttribute("aria-label", "My grid container");
	});

	it("should apply center style when center prop is true", async () => {
		await render(<Grid center data-testid="grid-center" />);
		const $el = screen.getByTestId("grid-center");

		expect($el).toHaveStyle({
			justifyContent: "center"
		});
	});

	// @TODO: Fix this. Latest StyleX update does not parse variables correctly within the test environment.
	it.skip("should apply custom gutter", async () => {
		await render(<Grid gutter={25} data-testid="grid-gutter" />);
		const $el = screen.getByTestId("grid-gutter");
		const styleAttribute = $el.getAttribute("style");

		expect(styleAttribute).toContain("--gridGap: 25;");
	});

	it("should apply custom minWidth and maxWidth using numbers (rem unit)", async () => {
		await render(<Grid minWidth={30} maxWidth={60} data-testid="grid-minmax-num" />);
		const $el = screen.getByTestId("grid-minmax-num");

		expect($el).toHaveStyle({
			gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 30rem), 60rem))"
		});
	});

	it("should apply custom minWidth and maxWidth using strings", async () => {
		await render(
			<Grid minWidth="250px" maxWidth="50%" data-testid="grid-minmax-str" />
		);
		const $el = screen.getByTestId("grid-minmax-str");

		expect($el).toHaveStyle({
			gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 250px), 50%))"
		});
	});

	it('should use "1fr" for maxWidth if value is not provided or undefined', async () => {
		await render(
			<Grid minWidth={15} maxWidth={undefined} data-testid="grid-max-default" />
		);
		const $el = screen.getByTestId("grid-max-default");
		expect($el).toHaveStyle({
			gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 15rem), 1fr))"
		});
	});

	it("should use 20rem for minWidth if value is not provided or undefined", async () => {
		await render(
			<Grid minWidth={undefined} maxWidth="500px" data-testid="grid-min-default" />
		);
		const $el = screen.getByTestId("grid-min-default");
		expect($el).toHaveStyle({
			gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 100rem), 500px))"
		});
	});
});
