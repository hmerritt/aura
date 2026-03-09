// For toHaveStyle, etc.
import * as stylex from "@stylexjs/stylex";
import { screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { testBasicComponent } from "tests/macros";
import { renderBasic as render } from "tests/render";

import { FlexProps } from "view/components";

import { Stack } from "./index";

vi.mock("view/components", async (importOriginal) => {
	const actual = await importOriginal<typeof import("view/components")>();
	return {
		...actual,
		Flex: vi.fn(
			({
				children,
				sx,
				style,
				className,
				row: _,
				...rest
			}: FlexProps & { sx?: any }) => {
				const resolvedProps = stylex.props(sx);
				const mergedStyle = { ...style, ...resolvedProps.style };
				const mergedClassName =
					`${className || ""} ${resolvedProps.className || ""}`.trim();

				return (
					<div
						style={mergedStyle}
						className={mergedClassName}
						data-testid="mock-flex"
						{...rest}
					>
						{children}
					</div>
				);
			}
		)
	};
});

const stackStyles = {
	stack0: {
		gap: "0"
	},
	stack1: {
		gap: ".2rem"
	},
	stack2: {
		gap: ".5rem"
	},
	stack3: {
		gap: "1rem"
	},
	stack4: {
		gap: "1.5rem"
	},
	stack5: {
		gap: "2rem"
	},
	stack6: {
		gap: "2.5rem"
	},
	stack7: {
		gap: "3rem"
	},
	stack8: {
		gap: "3.5rem"
	},
	stack9: {
		gap: "4rem"
	},
	stack10: {
		gap: "4.5rem"
	},
	stack11: {
		gap: "5rem"
	},
	stack12: {
		gap: "5.5rem"
	},
	stack13: {
		gap: "6rem"
	},
	stack14: {
		gap: "6.5rem"
	},
	stack15: {
		gap: "7rem"
	}
};

const customStyles = stylex.create({
	override: {
		backgroundColor: "blue",
		padding: "10px"
	}
});

describe("Stack component", () => {
	testBasicComponent({
		name: "(default)",
		Component: Stack,
		props: {
			spacing: 1
		},
		hasChildren: true,
		hasSx: true,
		shouldHaveStyles: {
			display: "flex",
			flexDirection: "column",
			gap: stackStyles.stack1.gap
		}
	});

	test("applies row direction when row prop is true", async () => {
		await render(<Stack row>Child</Stack>);
		const $el = screen.getByTestId("mock-flex");

		expect($el).toHaveStyle("flex-direction: row");
	});

	test.each([
		{ spacing: 1, expectedGap: stackStyles.stack1.gap },
		{ spacing: 3, expectedGap: stackStyles.stack3.gap },
		{ spacing: 5, expectedGap: stackStyles.stack5.gap },
		{ spacing: 10, expectedGap: stackStyles.stack10.gap },
		{ spacing: 15, expectedGap: stackStyles.stack15.gap }
	] as const)(
		"applies correct gap for spacing $spacing",
		async ({ spacing, expectedGap }) => {
			await render(<Stack spacing={spacing}>Child</Stack>);
			const $el = screen.getByTestId("mock-flex");

			expect($el).toHaveStyle(`gap: ${expectedGap}`);
		}
	);

	test("merges custom sx styles with default styles", async () => {
		await render(
			<Stack spacing={2} sx={customStyles.override}>
				Child
			</Stack>
		);
		const $el = screen.getByTestId("mock-flex");

		// Check default styles are still applied
		expect($el).toHaveStyle("display: flex");
		expect($el).toHaveStyle("flex-direction: column");
		expect($el).toHaveStyle(`gap: ${stackStyles.stack2.gap}`); // Default spacing

		// Check custom styles are applied
		expect($el).toHaveStyle("background-color: rgb(0, 0, 255)");
		expect($el).toHaveStyle("padding: 10px");
	});

	test("merges custom sx styles with specified props (row=true, spacing=5)", async () => {
		await render(
			<Stack row spacing={5} sx={customStyles.override}>
				Child
			</Stack>
		);
		const $el = screen.getByTestId("mock-flex");

		// Check specified prop styles are applied
		expect($el).toHaveStyle("display: flex");
		expect($el).toHaveStyle("flex-direction: row");
		expect($el).toHaveStyle(`gap: ${stackStyles.stack5.gap}`);

		// Check custom styles are applied and potentially override (though none conflict here)
		expect($el).toHaveStyle("background-color: rgb(0, 0, 255)");
		expect($el).toHaveStyle("padding: 10px");
	});

	test("passes through other props to the underlying Flex component", async () => {
		await render(
			<Stack id="my-stack" data-custom="value" aria-label="My Stack Section">
				Child
			</Stack>
		);
		const $el = screen.getByTestId("mock-flex");

		expect($el).toHaveAttribute("id", "my-stack");
		expect($el).toHaveAttribute("data-custom", "value");
		expect($el).toHaveAttribute("aria-label", "My Stack Section");
	});
});
