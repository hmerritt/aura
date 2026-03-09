import { describe } from "vitest";

import { testBasicComponent } from "tests/macros";

import { Image, ImageProps } from "./index";

describe("Image component", () => {
	(global as any).env = { isProd: false };

	testBasicComponent<ImageProps>({
		name: "(default)",
		Component: Image,
		props: {},
		hasSx: true,
		shouldNotHaveStyles: {
			height: "auto",
			maxWidth: "100%",
			display: "none",
			opacity: 0
		}
	});

	testBasicComponent<ImageProps>({
		name: "aspectRatioMaintain",
		Component: Image,
		props: {
			width: 100,
			height: 100,
			aspectRatioMaintain: true
		},
		hasSx: true,
		shouldHaveStyles: {
			height: "auto",
			maxWidth: "100%"
		},
		shouldNotHaveStyles: {
			display: "none",
			opacity: 0
		}
	});
});
