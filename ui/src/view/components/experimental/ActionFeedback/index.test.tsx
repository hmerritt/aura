import { act, fireEvent, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { renderBasic } from "tests/render";

type ActionFeedbackModule = typeof import("./index");

const loadActionFeedbackModule = async (
	mobile = false
): Promise<ActionFeedbackModule> => {
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

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.restoreAllMocks();
	vi.doUnmock("lib/device");
	vi.runOnlyPendingTimers();
	vi.useRealTimers();
});

describe("ActionFeedback", () => {
	test("triggerActionFeedback is safe without a mounted provider", async () => {
		const { triggerActionFeedback } = await loadActionFeedbackModule(false);

		expect(() => {
			triggerActionFeedback({
				element: <div>safe trigger</div>
			});
		}).not.toThrow();
	});

	test("renders feedback and follows cursor while active", async () => {
		const { ActionFeedbackProvider, triggerActionFeedback } =
			await loadActionFeedbackModule(false);

		await renderBasic(
			<ActionFeedbackProvider>
				<div>app</div>
			</ActionFeedbackProvider>,
			true
		);

		fireEvent.mouseMove(window, { clientX: 100, clientY: 120 });

		act(() => {
			triggerActionFeedback({
				element: <div data-testid="feedback-item">Saved</div>
			});
			vi.advanceTimersByTime(1);
		});

		expect(screen.getByTestId("feedback-item")).toBeInTheDocument();
		const $layer = screen.getByTestId("action-feedback-layer");
		expect($layer).toHaveAttribute("data-action-feedback-x", "100");
		expect($layer).toHaveAttribute("data-action-feedback-y", "120");
		expect($layer).toHaveAttribute("data-action-feedback-phase", "enter");

		fireEvent.mouseMove(window, { clientX: 222, clientY: 210 });
		expect($layer).toHaveAttribute("data-action-feedback-x", "222");
		expect($layer).toHaveAttribute("data-action-feedback-y", "210");
	});

	test("hides feedback after duration and exit animation", async () => {
		const { ActionFeedbackProvider, triggerActionFeedback } =
			await loadActionFeedbackModule(false);

		await renderBasic(
			<ActionFeedbackProvider>
				<div>app</div>
			</ActionFeedbackProvider>,
			true
		);

		act(() => {
			triggerActionFeedback({
				duration: 100,
				element: <div data-testid="feedback-timeout">Done</div>
			});
			vi.advanceTimersByTime(1);
		});

		expect(screen.getByTestId("feedback-timeout")).toBeInTheDocument();

		act(() => {
			vi.advanceTimersByTime(100);
		});

		expect(screen.getByTestId("action-feedback-layer")).toHaveAttribute(
			"data-action-feedback-phase",
			"exit"
		);

		act(() => {
			vi.advanceTimersByTime(180);
		});

		expect(screen.queryByTestId("feedback-timeout")).not.toBeInTheDocument();
		expect(screen.queryByTestId("action-feedback-layer")).not.toBeInTheDocument();
	});

	test("replaces current feedback with the latest trigger", async () => {
		const { ActionFeedbackProvider, triggerActionFeedback } =
			await loadActionFeedbackModule(false);

		await renderBasic(
			<ActionFeedbackProvider>
				<div>app</div>
			</ActionFeedbackProvider>,
			true
		);

		act(() => {
			triggerActionFeedback({
				duration: 800,
				element: <div data-testid="feedback-first">First</div>
			});
			vi.advanceTimersByTime(1);
		});

		expect(screen.getByTestId("feedback-first")).toBeInTheDocument();

		act(() => {
			triggerActionFeedback({
				duration: 800,
				element: <div data-testid="feedback-second">Second</div>
			});
			vi.advanceTimersByTime(1);
		});

		expect(screen.queryByTestId("feedback-first")).not.toBeInTheDocument();
		expect(screen.getByTestId("feedback-second")).toBeInTheDocument();
	});

	test("does not render feedback on mobile", async () => {
		const { ActionFeedbackProvider, triggerActionFeedback } =
			await loadActionFeedbackModule(true);

		await renderBasic(
			<ActionFeedbackProvider>
				<div>app</div>
			</ActionFeedbackProvider>,
			true
		);

		act(() => {
			triggerActionFeedback({
				element: <div data-testid="feedback-mobile">Mobile</div>
			});
			vi.advanceTimersByTime(1000);
		});

		expect(screen.queryByTestId("feedback-mobile")).not.toBeInTheDocument();
		expect(screen.queryByTestId("action-feedback-layer")).not.toBeInTheDocument();
	});
});
