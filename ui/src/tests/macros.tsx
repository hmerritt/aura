import * as stylex from "@stylexjs/stylex";
import { screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { render } from "./render";

const styles = stylex.create({
	border: { borderColor: "red", borderStyle: "solid", borderWidth: "1px" },
	override: {
		backgroundColor: "blue",
		padding: "10px"
	},
	test: {
		backgroundColor: "rgb(0, 0, 255)",
		padding: "1rem"
	}
});

/**
 * Performs basic tests for a given component.
 */
export function testBasicComponent<Props extends Record<string, any>>({
	name,
	Component,
	props,
	hasChildren,
	hasSx,
	shouldHaveStyles,
	shouldNotHaveStyles,
	shouldContainInStyleAttribute,
	shouldNotContainInStyleAttribute
}: {
	name: string;
	Component: any;
	/** Props to pass to component (for every test)  */
	props?: Props;
	/** true if Component can render children  */
	hasChildren?: boolean;
	/** true if Component has `sx` prop  */
	hasSx?: boolean;
	/** Object of expected styles  */
	shouldHaveStyles?: Record<string, any>;
	/** Object of styles that should NOT be present  */
	shouldNotHaveStyles?: Record<string, any>;
	/** Array of expected style attributes (used for styleX dynamic values)  */
	shouldContainInStyleAttribute?: `${string};`[];
	/** Array of style attributes that should NOT be present  */
	shouldNotContainInStyleAttribute?: `${string};`[];
}) {
	describe(`${name} - basic component tests`, () => {
		const testId = `component-${name}`;

		test("should render without crashing", async () => {
			await render(<Component {...props} data-testid={testId} />);
			const $el = screen.getByTestId(testId);

			expect($el).toBeInTheDocument();
		});

		if (hasChildren) {
			test("should render with children correctly", async () => {
				await render(
					<Component {...props}>
						<div>Child 1</div>
						<span>Child 2</span>
					</Component>
				);
				expect(screen.getByText("Child 1")).toBeInTheDocument();
				expect(screen.getByText("Child 2")).toBeInTheDocument();
			});
		}

		if (hasSx) {
			// @Note: Does not pass, when it should
			test.skip("should apply sx prop styles", async () => {
				await render(
					<Component {...props} sx={styles.test} data-testid={testId} />
				);
				const $el = screen.getByTestId(testId);

				expect($el).toHaveStyle({
					backgroundColor: "rgb(0, 0, 255)",
					padding: "1rem"
				});
			});

			// @Note: Passes even when the `toHaveStyle` value is WRONG
			test.skip("handles sx prop being an array", async () => {
				await render(
					<Component
						{...props}
						sx={[styles.override, styles.border]}
						data-testid={testId}
					/>
				);
				const $el = screen.getByTestId(testId);

				// Check styles from both items in the sx array
				expect($el).toHaveStyle("background-color: blue");
				expect($el).toHaveStyle("padding: 10px");
				expect($el).toHaveStyle("borderWidth: 1px");
				expect($el).toHaveStyle("borderStyle: solid");
				expect($el).toHaveStyle("borderColor: red");
			});
		}

		if (shouldHaveStyles) {
			test("should have expected styles", async () => {
				await render(<Component {...props} data-testid={testId} />);
				const $el = screen.getByTestId(testId);

				expect($el).toHaveStyle(shouldHaveStyles);
			});
		}
		if (shouldNotHaveStyles) {
			test("should NOT have unexpected styles", async () => {
				await render(<Component {...props} data-testid={testId} />);
				const $el = screen.getByTestId(testId);

				expect($el).not.toHaveStyle(shouldNotHaveStyles);
			});
		}

		if (shouldContainInStyleAttribute?.length) {
			test.skip("should apply expected styles in style attribute", async () => {
				await render(<Component {...props} data-testid={testId} />);
				const $el = screen.getByTestId(testId);
				const styleAttribute = $el.getAttribute("style");

				for (const style of shouldContainInStyleAttribute) {
					expect(styleAttribute).toContain(style);
				}
			});
		}
		if (shouldNotContainInStyleAttribute?.length) {
			test("should NOT apply unexpected styles in style attribute", async () => {
				await render(<Component {...props} data-testid={testId} />);
				const $el = screen.getByTestId(testId);
				const styleAttribute = $el.getAttribute("style");

				for (const style of shouldNotContainInStyleAttribute) {
					expect(styleAttribute).not.toContain(style);
				}
			});
		}
	});
}
