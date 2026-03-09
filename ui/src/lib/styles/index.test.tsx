import * as stylex from "@stylexjs/stylex";
import { cleanStyle, getStyle, render, selectTestId } from "tests";
import { describe, expect, test } from "vitest";

import { colors } from "../../lib/styles/colors.stylex";
import { shadowFn, shadows } from "../../lib/styles/shadows.stylex";
import { variables } from "../../lib/styles/variables.stylex";

/**
 * Mini mock component to test StyleX styles are being compiled correctly.
 *
 * @TODO E2E tests
 */

export const StylesMock = () => (
	<div
		data-testid="StylesMock"
		{...stylex.props(styles.container, styles.shadow, styles.variable)}
	>
		<h1
			data-testid="title"
			{...stylex.props(styles.title, shadowFn.textBlock(colors.test1))}
		>
			Title
		</h1>
		<h2 data-testid="sub-title" {...stylex.props(styles.subTitle)}>
			Sub Title
		</h2>
		<span data-testid="box-shadow" {...stylex.props(shadowFn.boxBlock(colors.test2))}>
			Box Shadow Fn
		</span>
	</div>
);

const styles = stylex.create({
	container: {
		marginLeft: "auto",
		marginRight: "auto",
		maxWidth: "567px",
		transition: "all, 80ms, ease"
	},
	shadow: {
		boxShadow: shadows.test2
	},
	subTitle: {
		color: colors.test2
	},
	title: {
		color: colors.test1
	},
	variable: {
		width: variables.test1
	}
});

describe("StyleX theme", () => {
	// @TODO: Fix this. Latest StyleX update does not parse variables correctly within the test environment.
	test.skip("renders colors", async () => {
		const { container } = await render(<StylesMock />);

		const styleTitle = getStyle(selectTestId(container, "title"));
		expect(styleTitle.color).toBe("#38A169");

		const styleSubTitle = getStyle(selectTestId(container, "sub-title"));
		expect(styleSubTitle.color).toBe("#DD6B20");
	});

	test("renders mixins", async () => {
		const { container } = await render(<StylesMock />);

		const styleContainer = getStyle(selectTestId(container, "StylesMock"));
		expect(styleContainer.maxWidth).toBe("567px");
		expect(styleContainer.marginLeft).toBe("auto");
		expect(styleContainer.marginRight).toBe("auto");
		expect(styleContainer.transition).toBe("all,.08s,ease");
	});

	// @TODO: Fix this. Latest StyleX update does not parse variables correctly within the test environment.
	test.skip("renders shadows", async () => {
		const { container } = await render(<StylesMock />);

		const styleContainer = getStyle(selectTestId(container, "StylesMock"));
		expect(cleanStyle(styleContainer.boxShadow)).toBe(
			"0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24)"
		);

		const styleTitle = getStyle(selectTestId(container, "title"));
		expect(cleanStyle(styleTitle.textShadow)).toBe(
			"0.25px 0.25px 0 #38A169, 0.5px 0.5px 0 #38A169, 0.75px 0.75px 0 #38A169, 1px 1px 0 #38A169, 1.25px 1.25px 0 #38A169, 1.5px 1.5px 0 #38A169, 1.75px 1.75px 0 #38A169, 2px 2px 0 #38A169, 2.25px 2.25px 0 #38A169, 2.5px 2.5px 0 #38A169, 2.75px 2.75px 0 #38A169, 3px 3px 0 #38A169, 3.25px 3.25px 0 #38A169, 3.5px 3.5px 0 #38A169, 3.75px 3.75px 0 #38A169, 4px 4px 0 #38A169, 4.25px 4.25px 0 #38A169, 4.5px 4.5px 0 #38A169, 4.75px 4.75px 0 #38A169, 5px 5px 0 #38A169, 5.25px 5.25px 0 #38A169, 5.5px 5.5px 0 #38A169, 5.75px 5.75px 0 #38A169, 6px 6px 0 #38A169"
		);

		const styleBoxShadow = getStyle(selectTestId(container, "box-shadow"));
		expect(cleanStyle(styleBoxShadow.boxShadow)).toBe(
			"0.25px 0.25px 0 #DD6B20, 0.5px 0.5px 0 #DD6B20, 0.75px 0.75px 0 #DD6B20, 1px 1px 0 #DD6B20, 1.25px 1.25px 0 #DD6B20, 1.5px 1.5px 0 #DD6B20, 1.75px 1.75px 0 #DD6B20, 2px 2px 0 #DD6B20, 2.25px 2.25px 0 #DD6B20, 2.5px 2.5px 0 #DD6B20, 2.75px 2.75px 0 #DD6B20, 3px 3px 0 #DD6B20, 3.25px 3.25px 0 #DD6B20, 3.5px 3.5px 0 #DD6B20, 3.75px 3.75px 0 #DD6B20, 4px 4px 0 #DD6B20, 4.25px 4.25px 0 #DD6B20, 4.5px 4.5px 0 #DD6B20, 4.75px 4.75px 0 #DD6B20, 5px 5px 0 #DD6B20, 5.25px 5.25px 0 #DD6B20, 5.5px 5.5px 0 #DD6B20, 5.75px 5.75px 0 #DD6B20, 6px 6px 0 #DD6B20"
		);
	});

	test.skip("renders variables", async () => {
		const { container } = await render(<StylesMock />);

		const styleContainer = getStyle(selectTestId(container, "StylesMock"));
		expect(styleContainer.width).toBe("5678px");
	});
	test("renders variables (does NOT test what the value is)", async () => {
		const { container } = await render(<StylesMock />);

		const styleContainer = getStyle(selectTestId(container, "StylesMock"));
		const isVariable = /var\(--([a-zA-Z0-9]+)\)/.test(styleContainer.width);
		expect(isVariable).toBe(true);
	});
});
