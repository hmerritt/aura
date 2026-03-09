import { describe } from "vitest";

import { testBasicComponent } from "tests/macros";

import { Flex, FlexProps } from "./index";

describe("Flex component", () => {
	testBasicComponent<FlexProps>({
		name: "(default)",
		Component: Flex,
		props: {},
		hasChildren: true,
		hasSx: true,
		shouldHaveStyles: {
			display: "flex",
			flexWrap: "nowrap",
			flexDirection: "column",
			minWidth: 0
		}
	});
});
