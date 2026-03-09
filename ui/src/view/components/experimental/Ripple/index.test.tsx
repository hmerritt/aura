import { act, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { renderBasic } from "tests/render";

import { Ripple } from "./index";

const getRippleCircle = (root: HTMLElement) =>
	root.querySelector("[data-ripple] > span") as HTMLSpanElement | null;

describe("Ripple component", () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.runOnlyPendingTimers();
		vi.useRealTimers();
	});

	test("keeps ripple visible while holding, then clears on release", async () => {
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple">
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 12,
			clientY: 18,
			pointerId: 1,
			pointerType: "mouse"
		});
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(1);

		act(() => {
			vi.advanceTimersByTime(3000);
		});
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(1);

		fireEvent.pointerUp(root, {
			clientX: 12,
			clientY: 18,
			pointerId: 1,
			pointerType: "mouse"
		});

		act(() => {
			vi.advanceTimersByTime(300);
		});
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(0);
	});

	test("clears ripple on pointer cancel", async () => {
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple">
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 10,
			clientY: 10,
			pointerId: 2,
			pointerType: "mouse"
		});
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(1);

		fireEvent.pointerCancel(root, {
			pointerId: 2,
			pointerType: "mouse"
		});

		act(() => {
			vi.advanceTimersByTime(300);
		});
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(0);
	});

	test("uses default ripple color", async () => {
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple">
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 9,
			clientY: 9,
			pointerId: 21,
			pointerType: "mouse"
		});

		const ripple = getRippleCircle(root);
		expect(ripple).not.toBeNull();
		expect(ripple?.style.backgroundColor).toBe("rgba(0, 0, 0, 0.1)");
	});

	test("uses custom ripple color", async () => {
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple" color="rgba(255, 0, 0, 0.4)">
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 11,
			clientY: 11,
			pointerId: 22,
			pointerType: "mouse"
		});

		const ripple = getRippleCircle(root);
		expect(ripple).not.toBeNull();
		expect(ripple?.style.backgroundColor).toBe("rgba(255, 0, 0, 0.4)");
	});

	test("uses hoverColor when hoverBg is enabled", async () => {
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple" hoverBg hoverColor="#ededed">
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		expect(root).toHaveStyle("background-color: var(--x-backgroundColor)");
		expect(root.getAttribute("style")).toContain("#ededed");
	});

	test("does not create ripples when disabled", async () => {
		const onPointerDown = vi.fn();
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple" disabled onPointerDown={onPointerDown}>
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 8,
			clientY: 8,
			pointerId: 3,
			pointerType: "mouse"
		});

		expect(onPointerDown).not.toHaveBeenCalled();
		expect(root.querySelectorAll("[data-ripple]")).toHaveLength(0);
	});

	test("invokes legacy mouse handlers from pointer events", async () => {
		const onMouseDown = vi.fn();
		const onMouseUp = vi.fn();
		const { getByTestId } = await renderBasic(
			<Ripple data-testid="ripple" onMouseDown={onMouseDown} onMouseUp={onMouseUp}>
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 18,
			clientY: 20,
			pointerId: 4,
			pointerType: "mouse"
		});
		fireEvent.pointerUp(root, {
			clientX: 18,
			clientY: 20,
			pointerId: 4,
			pointerType: "mouse"
		});

		expect(onMouseDown).toHaveBeenCalledTimes(1);
		expect(onMouseUp).toHaveBeenCalledTimes(1);
	});

	test("invokes legacy touch handlers from pointer events", async () => {
		const onTouchStart = vi.fn();
		const onTouchEnd = vi.fn();
		const { getByTestId } = await renderBasic(
			<Ripple
				data-testid="ripple"
				onTouchStart={onTouchStart}
				onTouchEnd={onTouchEnd}
			>
				<button>Ripple</button>
			</Ripple>,
			true
		);
		const root = getByTestId("ripple");

		fireEvent.pointerDown(root, {
			clientX: 14,
			clientY: 14,
			pointerId: 5,
			pointerType: "touch"
		});
		fireEvent.pointerUp(root, {
			clientX: 14,
			clientY: 14,
			pointerId: 5,
			pointerType: "touch"
		});

		expect(onTouchStart).toHaveBeenCalledTimes(1);
		expect(onTouchEnd).toHaveBeenCalledTimes(1);
	});
});
