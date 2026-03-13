import * as stylex from "@stylexjs/stylex";
import { act, fireEvent, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { renderBasic } from "tests/render";

import { Loader } from "./index";

const compact = (value: string) => value.replaceAll(" ", "");

const styles = stylex.create({
    sizeOverride: {
        height: "2rem",
        width: "2rem"
    }
});

describe("Loader component", () => {
    test("renders dotgrid loader by default shape and defaults", async () => {
        await renderBasic(<Loader type="dotgrid" data-testid="loader" />);

        const $loader = screen.getByTestId("loader");
        const computed = window.getComputedStyle($loader);

        expect($loader).toHaveAttribute("data-loader-type", "dotgrid");
        expect($loader.querySelectorAll('[data-loader-dot="slot"]')).toHaveLength(9);
        expect($loader.querySelectorAll('[data-loader-dot="trail"]')).toHaveLength(3);
        expect($loader).toHaveStyle("width: var(--x-width)");
        expect(compact(computed.getPropertyValue("--x-width"))).toBe("1.25rem");
        expect($loader).toHaveAttribute("data-loader-duration-ms", "900");
        expect($loader).toHaveAttribute("data-loader-color", "currentColor");
    });

    test("applies custom size, speed, and color", async () => {
        await renderBasic(
            <Loader
                type="dotgrid"
                size={24}
                speed={2}
                color="tomato"
                data-testid="loader"
            />
        );

        const $loader = screen.getByTestId("loader");
        const computed = window.getComputedStyle($loader);

        expect($loader).toHaveStyle("width: var(--x-width)");
        expect(compact(computed.getPropertyValue("--x-width"))).toBe("24px");
        expect($loader).toHaveAttribute("data-loader-duration-ms", "450");
        expect($loader).toHaveAttribute("data-loader-color", "tomato");
    });

    test("falls back to default speed when speed is invalid", async () => {
        await renderBasic(<Loader type="dotgrid" speed={0} data-testid="loader" />);

        const $loader = screen.getByTestId("loader");

        expect($loader).toHaveAttribute("data-loader-duration-ms", "900");
    });

    test("forwards native div props", async () => {
        const onClick = vi.fn();
        await renderBasic(
            <Loader
                type="dotgrid"
                id="loader-id"
                title="loading"
                aria-label="Loading content"
                data-state="busy"
                data-testid="loader"
                onClick={onClick}
            />
        );

        const $loader = screen.getByTestId("loader");

        expect($loader).toHaveAttribute("id", "loader-id");
        expect($loader).toHaveAttribute("title", "loading");
        expect($loader).toHaveAttribute("aria-label", "Loading content");
        expect($loader).toHaveAttribute("data-state", "busy");

        fireEvent.click($loader);
        expect(onClick).toHaveBeenCalledTimes(1);
    });

    test("allows sx to override generated css vars", async () => {
        await renderBasic(
            <Loader
                type="dotgrid"
                size={20}
                sx={styles.sizeOverride}
                data-testid="loader"
            />
        );

        const $loader = screen.getByTestId("loader");
        const computed = window.getComputedStyle($loader);

        expect(computed.width).toBe("2rem");
    });

    test("animates to center, converges all dots, fades out, pauses, then restarts", async () => {
        vi.useFakeTimers();
        try {
            await renderBasic(<Loader type="dotgrid" data-testid="loader" />, true);

            const $loader = screen.getByTestId("loader");
            const getTrailState = () => {
                const $trailDots = Array.from(
                    $loader.querySelectorAll('[data-loader-dot="trail"]')
                );
                return {
                    opacity: Number($trailDots[0]?.getAttribute("data-loader-opacity")),
                    phase: $trailDots[0]?.getAttribute("data-loader-phase"),
                    positions: $trailDots.map(($el) =>
                        $el.getAttribute("data-loader-position")
                    )
                };
            };

            const expectedFrames = [
                { opacity: 1, phase: "snake", positions: ["dot0", "dot0", "dot0"] },
                { opacity: 1, phase: "snake", positions: ["dot1", "dot0", "dot0"] },
                { opacity: 1, phase: "snake", positions: ["dot2", "dot1", "dot0"] },
                { opacity: 1, phase: "snake", positions: ["dot5", "dot2", "dot1"] },
                { opacity: 1, phase: "snake", positions: ["dot8", "dot5", "dot2"] },
                { opacity: 1, phase: "snake", positions: ["dot7", "dot8", "dot5"] },
                { opacity: 1, phase: "snake", positions: ["dot6", "dot7", "dot8"] },
                { opacity: 1, phase: "snake", positions: ["dot3", "dot6", "dot7"] },
                { opacity: 1, phase: "snake", positions: ["dot4", "dot3", "dot6"] },
                { opacity: 1, phase: "converge", positions: ["dot4", "dot4", "dot3"] },
                { opacity: 1, phase: "converge", positions: ["dot4", "dot4", "dot4"] },
                { opacity: 0.66, phase: "fade", positions: ["dot4", "dot4", "dot4"] },
                { opacity: 0.33, phase: "fade", positions: ["dot4", "dot4", "dot4"] },
                { opacity: 0, phase: "fade", positions: ["dot4", "dot4", "dot4"] },
                { opacity: 0, phase: "pause", positions: ["dot4", "dot4", "dot4"] },
                { opacity: 1, phase: "snake", positions: ["dot0", "dot0", "dot0"] },
                { opacity: 1, phase: "snake", positions: ["dot1", "dot0", "dot0"] }
            ] as const;

            const stepMs = 90; // 900ms default duration / 10 frames
            expect(getTrailState()).toEqual(expectedFrames[0]);

            for (const expectedFrame of expectedFrames.slice(1)) {
                act(() => {
                    vi.advanceTimersByTime(stepMs);
                });
                expect(getTrailState()).toEqual(expectedFrame);
            }
        } finally {
            vi.useRealTimers();
        }
    });
});
