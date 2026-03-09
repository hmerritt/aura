import * as stylex from "@stylexjs/stylex";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import { renderBasic } from "tests/render";

type HaloModule = typeof import("./index");
type HaloSide = "top" | "right" | "bottom" | "left";

const compact = (value: string) => value.replaceAll(" ", "");
const testStyles = stylex.create({
	radiusOverride: {
		borderRadius: "22px"
	}
});

const mockRect = (element: HTMLElement, rect: Partial<DOMRect>) => {
	const fullRect = {
		x: rect.left ?? 0,
		y: rect.top ?? 0,
		top: rect.top ?? 0,
		left: rect.left ?? 0,
		right: rect.right ?? 0,
		bottom: rect.bottom ?? 0,
		width: rect.width ?? (rect.right ?? 0) - (rect.left ?? 0),
		height: rect.height ?? (rect.bottom ?? 0) - (rect.top ?? 0),
		toJSON: () => ({})
	} as DOMRect;

	vi.spyOn(element, "getBoundingClientRect").mockReturnValue(fullRect);
};

const loadHaloModule = async (mobile = false): Promise<HaloModule> => {
	vi.resetModules();
	vi.doMock("lib/device", () => ({
		isMobile: mobile,
		isTablet: false,
		userAgent: mobile ? "mobile" : "desktop",
		parseUserAgent: () => ({
			ua: mobile ? "mobile" : "desktop",
			isMobile: mobile,
			isTablet: false
		})
	}));

	return import("./index");
};

afterEach(() => {
	vi.restoreAllMocks();
	vi.doUnmock("lib/device");
});

describe("Halo component", () => {
	test.each([
		{
			name: "defaults to 1px on all sides when sides are not provided",
			lineSize: undefined,
			sides: undefined,
			expected: {
				top: "1px",
				right: "1px",
				bottom: "1px",
				left: "1px"
			}
		},
		{
			name: "uses lineSize for truthy sides and 0px for false sides",
			lineSize: "3px",
			sides: { top: true, right: false, bottom: true, left: false },
			expected: {
				top: "3px",
				right: "0px",
				bottom: "3px",
				left: "0px"
			}
		},
		{
			name: "treats unspecified sides as false when sides object is partial",
			lineSize: "4px",
			sides: { left: true },
			expected: {
				top: "0px",
				right: "0px",
				bottom: "0px",
				left: "4px"
			}
		}
	])("$name", async ({ lineSize, sides, expected }) => {
		const { Halo } = await loadHaloModule(false);
		await renderBasic(
			<Halo
				data-testid="halo-sides"
				lineSize={lineSize}
				sides={sides as Partial<Record<HaloSide, boolean>> | undefined}
			>
				<div>Item</div>
			</Halo>
		);

		const $el = screen.getByTestId("halo-sides");
		const computed = getComputedStyle($el);
		expect($el).toHaveStyle("padding-top: var(--x-paddingTop)");
		expect($el).toHaveStyle("padding-right: var(--x-paddingRight)");
		expect($el).toHaveStyle("padding-bottom: var(--x-paddingBottom)");
		expect($el).toHaveStyle("padding-left: var(--x-paddingLeft)");
		expect(compact(computed.getPropertyValue("--x-paddingTop"))).toBe(expected.top);
		expect(compact(computed.getPropertyValue("--x-paddingRight"))).toBe(expected.right);
		expect(compact(computed.getPropertyValue("--x-paddingBottom"))).toBe(expected.bottom);
		expect(compact(computed.getPropertyValue("--x-paddingLeft"))).toBe(expected.left);
	});

	test("supports Halo props, inherited div props, and event handlers", async () => {
		const { Halo } = await loadHaloModule(false);
		const onClick = vi.fn();

		await renderBasic(
			<Halo
				data-testid="halo-props"
				id="halo-id"
				title="halo-title"
				size="30rem"
				halo="rgb(1, 2, 3)"
				lineSize="3px"
				sides={{ top: true, right: true, bottom: true, left: true }}
				onClick={onClick}
			>
				Text child
				<div data-testid="halo-child">Item</div>
			</Halo>
		);

		const $el = screen.getByTestId("halo-props");
		expect($el).toHaveAttribute("id", "halo-id");
		expect($el).toHaveAttribute("title", "halo-title");
		expect($el).toHaveAttribute("data-halo");
		expect($el).toHaveAttribute("data-halo-size", "30rem");
		expect($el).toHaveAttribute("data-halo-color", "rgb(1, 2, 3)");

		fireEvent.click($el);
		expect(onClick).toHaveBeenCalledTimes(1);
		expect($el).toHaveTextContent("Text child");
		expect(screen.getByTestId("halo-child")).toBeInTheDocument();
	});

	test("applies borderRadius to halo and all valid element children", async () => {
		const { Halo } = await loadHaloModule(false);

		await renderBasic(
			<Halo data-testid="halo-border-radius" borderRadius="14px">
				<div data-testid="halo-child-one">One</div>
				<section data-testid="halo-child-two">Two</section>
				Text child
			</Halo>
		);

		const $halo = screen.getByTestId("halo-border-radius");
		const $childOne = screen.getByTestId("halo-child-one");
		const $childTwo = screen.getByTestId("halo-child-two");
		const computed = getComputedStyle($halo);

		expect($halo.style.cssText).toContain("14px");
		expect($halo).toHaveStyle("overflow: var(--x-overflow)");
		expect(compact(computed.getPropertyValue("--x-overflow"))).toBe("hidden");
		expect($childOne.style.cssText).toContain("14px");
		expect($childTwo.style.cssText).toContain("14px");
	});

	test("allows sx borderRadius to override Halo borderRadius on wrapper only", async () => {
		const { Halo } = await loadHaloModule(false);

		await renderBasic(
			<Halo
				data-testid="halo-sx-radius"
				borderRadius="14px"
				sx={testStyles.radiusOverride}
			>
				<div data-testid="halo-sx-radius-child">Child</div>
			</Halo>
		);

		const $halo = screen.getByTestId("halo-sx-radius");
		const $child = screen.getByTestId("halo-sx-radius-child");

		expect(getComputedStyle($halo).borderRadius).toBe("22px");
		expect($child.style.cssText).toContain("14px");
		expect($child.style.cssText).not.toContain("22px");
	});
});

describe("HaloProvider", () => {
	test("applies radial gradients on desktop and resolves element/provider gradient values", async () => {
		const { Halo, HaloProvider } = await loadHaloModule(false);

		Object.defineProperty(window, "innerWidth", {
			configurable: true,
			value: 1200
		});
		Object.defineProperty(window, "innerHeight", {
			configurable: true,
			value: 800
		});

		await renderBasic(
			<HaloProvider gradient={{ size: "40rem", halo: "rgb(10, 20, 30)" }}>
				<Halo data-testid="halo-override" size="18rem" halo="rgb(1, 2, 3)">
					<div>Override</div>
				</Halo>
				<Halo data-testid="halo-provider-fallback">
					<div>Provider fallback</div>
				</Halo>
			</HaloProvider>
		);

		const $override = screen.getByTestId("halo-override");
		const $providerFallback = screen.getByTestId("halo-provider-fallback");

		mockRect($override, { top: 50, left: 60, right: 260, bottom: 160 });
		mockRect($providerFallback, { top: 200, left: 220, right: 420, bottom: 320 });

		fireEvent.mouseMove(window, { clientX: 250, clientY: 230 });

		await waitFor(() => {
			const background = compact($override.style.background);
			expect(background).toContain("radial-gradient(18rem");
			expect(background).toContain("rgb(1,2,3)");
		});

		await waitFor(() => {
			const background = compact($providerFallback.style.background);
			expect(background).toContain("radial-gradient(40rem");
			expect(background).toContain("rgb(10,20,30)");
		});
	});

	test("falls back to default provider gradient when no element or provider gradient is set", async () => {
		const { Halo, HaloProvider } = await loadHaloModule(false);

		await renderBasic(
			<HaloProvider>
				<Halo data-testid="halo-default-gradient">
					<div>Default fallback</div>
				</Halo>
			</HaloProvider>
		);

		const $el = screen.getByTestId("halo-default-gradient");
		mockRect($el, { top: 80, left: 100, right: 260, bottom: 240 });

		fireEvent.mouseMove(window, { clientX: 120, clientY: 140 });

		await waitFor(() => {
			const background = compact($el.style.background);
			expect(background).toContain("radial-gradient(24rem");
			expect(background).toContain("rgb(120,120,120)");
		});
	});

	test("recomputes gradient on scroll using the latest known mouse position", async () => {
		const { Halo, HaloProvider } = await loadHaloModule(false);

		await renderBasic(
			<HaloProvider>
				<Halo
					data-testid="halo-scroll-recompute"
					halo="rgb(11, 22, 33)"
					size="20rem"
				>
					<div>Scroll recompute</div>
				</Halo>
			</HaloProvider>
		);

		const $el = screen.getByTestId("halo-scroll-recompute");
		mockRect($el, { top: 10, left: 10, right: 210, bottom: 110 });

		fireEvent.mouseMove(window, { clientX: 120, clientY: 60 });

		await waitFor(() => {
			expect(compact($el.style.background)).toContain("radial-gradient(20rem");
		});

		$el.style.background = "";
		fireEvent.scroll(window);

		await waitFor(() => {
			const background = compact($el.style.background);
			expect(background).toContain("radial-gradient(20rem");
			expect(background).toContain("rgb(11,22,33)");
		});
	});

	test("uses static color and stops dynamic updates on mobile when staticForMobile is true", async () => {
		const { Halo, HaloProvider } = await loadHaloModule(true);

		await renderBasic(
			<HaloProvider
				staticForMobile
				gradient={{ size: "42rem", halo: "rgb(9, 9, 9)" }}
			>
				<Halo data-testid="halo-mobile-static">
					<div>Mobile static</div>
				</Halo>
			</HaloProvider>
		);

		const $el = screen.getByTestId("halo-mobile-static");

		await waitFor(() => {
			const background = compact($el.style.background);
			expect(background).toContain("rgb(9,9,9)");
			expect(background).not.toContain("radial-gradient(");
		});

		const initialBackground = $el.style.background;

		fireEvent.mouseMove(window, { clientX: 300, clientY: 300 });
		fireEvent.scroll(window);

		expect($el.style.background).toBe(initialBackground);
	});
});
