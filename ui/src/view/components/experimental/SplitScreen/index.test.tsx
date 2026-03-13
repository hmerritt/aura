import * as stylex from "@stylexjs/stylex";
import * as React from "react";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import { renderBasic } from "tests/render";

import { useSplitScreenHandle } from "./index";
import { clampDividers, movePaneInMode, normalizeAssignments } from "./layout";

vi.mock("animejs", () => ({
    animate: vi.fn()
}));

type RectInput = Partial<DOMRect> & { width?: number; height?: number };
type HandleProps = {
    onPointerDown: (evt: React.PointerEvent<HTMLElement>) => void;
    draggable: false;
    role: "button";
    tabIndex: 0;
    "aria-label": string;
    "data-split-screen-handle": "true";
    "data-split-screen-handle-pane-id": string;
};

const testStyles = stylex.create({
    rootOverride: {
        minHeight: "420px"
    }
});

const mockRect = (element: Element, rect: RectInput) => {
    const width = rect.width ?? Math.max(0, (rect.right ?? 0) - (rect.left ?? 0));
    const height = rect.height ?? Math.max(0, (rect.bottom ?? 0) - (rect.top ?? 0));
    const value = {
        x: rect.left ?? 0,
        y: rect.top ?? 0,
        top: rect.top ?? 0,
        left: rect.left ?? 0,
        right: rect.right ?? (rect.left ?? 0) + width,
        bottom: rect.bottom ?? (rect.top ?? 0) + height,
        width,
        height,
        toJSON: () => ({})
    } as DOMRect;

    return vi.spyOn(element, "getBoundingClientRect").mockReturnValue(value);
};

const StatefulPaneBase = ({
    id,
    handleProps
}: {
    id: string;
    handleProps?: HandleProps;
}) => {
    const [value, setValue] = React.useState("");

    return (
        <div>
            {handleProps ? (
                <button
                    {...handleProps}
                    data-testid={`custom-pane-handle-${id}`}
                    type="button"
                >
                    Move
                </button>
            ) : null}
            <label htmlFor={`input-${id}`}>Pane {id}</label>
            <input
                id={`input-${id}`}
                data-testid={`pane-input-${id}`}
                value={value}
                onChange={(evt) => setValue(evt.currentTarget.value)}
            />
        </div>
    );
};

const StatefulPane = ({ id }: { id: string }) => <StatefulPaneBase id={id} />;

const StatefulPaneWithHandle = ({ id }: { id: string }) => {
    const handleProps = useSplitScreenHandle(id);
    return <StatefulPaneBase id={id} handleProps={handleProps} />;
};

describe("SplitScreen layout helpers", () => {
    test("normalizeAssignments fills side and quad sectors from pane order", () => {
        const assignments = normalizeAssignments(["a", "b", "c", "d"]);

        expect(assignments.left).toBe("a");
        expect(assignments.right).toBe("b");
        expect(assignments.topLeft).toBe("a");
        expect(assignments.topRight).toBe("b");
        expect(assignments.bottomLeft).toBe("c");
        expect(assignments.bottomRight).toBe("d");
    });

    test("clampDividers respects min pane size", () => {
        const dividers = clampDividers(
            { xRatio: 0.05, yRatio: 0.95 },
            { width: 1000, height: 600 },
            120
        );

        expect(dividers.xRatio).toBeCloseTo(0.12);
        expect(dividers.yRatio).toBeCloseTo(0.8);
    });

    test("movePaneInMode swaps panes when target sector is occupied", () => {
        const result = movePaneInMode(
            {
                left: "a",
                right: "b"
            },
            "side-by-side",
            "a",
            "right"
        );

        expect(result.changed).toBe(true);
        expect(result.from).toBe("left");
        expect(result.to).toBe("right");
        expect(result.assignments.left).toBe("b");
        expect(result.assignments.right).toBe("a");
    });
});

describe("SplitScreen component", () => {
    test("renders side-by-side by default, allows sx overrides, and does not inject pane chrome", async () => {
        const { SplitScreen } = await import("./index");

        await renderBasic(
            <SplitScreen
                data-testid="split-screen-root"
                panes={[
                    { id: "a", node: <div>A</div> },
                    { id: "b", node: <div>B</div> }
                ]}
                sx={testStyles.rootOverride}
            />
        );

        const root = screen.getByTestId("split-screen-root");
        expect(root).toHaveAttribute("data-split-screen-mode", "side-by-side");
        expect(screen.getByTestId("split-screen-sector-left")).toBeInTheDocument();
        expect(screen.getByTestId("split-screen-sector-right")).toBeInTheDocument();
        expect(
            screen.queryByTestId("split-screen-sector-topLeft")
        ).not.toBeInTheDocument();
        expect(getComputedStyle(root).minHeight).toBe("420px");
        expect(
            screen.queryByTestId("split-screen-pane-handle-a")
        ).not.toBeInTheDocument();
        expect(screen.queryByText("Empty sector")).not.toBeInTheDocument();
    });

    test("switches to quad mode and renders horizontal divider", async () => {
        const { SplitScreen } = await import("./index");

        await renderBasic(
            <SplitScreen
                showModeControls
                panes={[
                    { id: "a", node: <div>A</div> },
                    { id: "b", node: <div>B</div> },
                    { id: "c", node: <div>C</div> },
                    { id: "d", node: <div>D</div> }
                ]}
            />
        );

        fireEvent.click(screen.getByTestId("split-screen-mode-quad"));

        await waitFor(() => {
            expect(
                screen.getByTestId("split-screen-divider-horizontal")
            ).toBeInTheDocument();
            expect(screen.getByTestId("split-screen-sector-topLeft")).toBeInTheDocument();
            expect(
                screen.getByTestId("split-screen-sector-bottomRight")
            ).toBeInTheDocument();
        });
    });

    test("drags the vertical divider and updates x ratio", async () => {
        const { SplitScreen } = await import("./index");
        const onLayoutChange = vi.fn();

        await renderBasic(
            <SplitScreen
                data-testid="split-screen-root"
                showModeControls
                onLayoutChange={onLayoutChange}
                panes={[
                    { id: "a", node: <div>A</div> },
                    { id: "b", node: <div>B</div> }
                ]}
            />
        );

        const root = screen.getByTestId("split-screen-root");
        mockRect(root, { left: 100, top: 50, width: 1000, height: 600 });

        fireEvent.pointerDown(screen.getByTestId("split-screen-divider-vertical"), {
            clientX: 600,
            clientY: 100
        });
        fireEvent.pointerMove(window, { clientX: 800, clientY: 100 });
        fireEvent.pointerUp(window, { clientX: 800, clientY: 100 });

        await waitFor(() => {
            expect(root.getAttribute("style")).toContain("70%");
        });
        expect(onLayoutChange).toHaveBeenCalled();
    });

    test("moves a pane via hook-based handle without losing input state", async () => {
        const { SplitScreen } = await import("./index");
        const onPaneMove = vi.fn();

        await renderBasic(
            <SplitScreen
                onPaneMove={onPaneMove}
                panes={[
                    {
                        id: "a",
                        node: <StatefulPaneWithHandle id="a" />
                    },
                    {
                        id: "b",
                        node: <StatefulPaneWithHandle id="b" />
                    }
                ]}
            />
        );

        const left = screen.getByTestId("split-screen-sector-left");
        const right = screen.getByTestId("split-screen-sector-right");
        mockRect(left, { left: 0, top: 0, right: 400, bottom: 300 });
        mockRect(right, { left: 401, top: 0, right: 800, bottom: 300 });

        const input = screen.getByTestId("pane-input-a") as HTMLInputElement;
        fireEvent.change(input, { target: { value: "persist me" } });
        expect(input.value).toBe("persist me");

        fireEvent.pointerDown(screen.getByTestId("custom-pane-handle-a"), {
            clientX: 100,
            clientY: 30
        });
        fireEvent.pointerMove(window, { clientX: 600, clientY: 120 });
        fireEvent.pointerUp(window, { clientX: 600, clientY: 120 });

        await waitFor(() => {
            expect((screen.getByTestId("pane-input-a") as HTMLInputElement).value).toBe(
                "persist me"
            );
        });

        expect(onPaneMove).toHaveBeenCalledWith({
            paneId: "a",
            from: "left",
            to: "right"
        });
    });

    test("preserves hidden pane state across mode switch when hiddenPaneBehavior is preserve", async () => {
        const { SplitScreen } = await import("./index");

        await renderBasic(
            <SplitScreen
                defaultMode="quad"
                showModeControls
                panes={[
                    {
                        id: "a",
                        node: <StatefulPaneWithHandle id="a" />
                    },
                    {
                        id: "b",
                        node: <StatefulPaneWithHandle id="b" />
                    },
                    {
                        id: "c",
                        node: <StatefulPaneWithHandle id="c" />
                    },
                    {
                        id: "d",
                        node: <StatefulPaneWithHandle id="d" />
                    }
                ]}
            />
        );

        const input = screen.getByTestId("pane-input-c") as HTMLInputElement;
        fireEvent.change(input, { target: { value: "keep hidden" } });
        expect(input.value).toBe("keep hidden");

        fireEvent.click(screen.getByTestId("split-screen-mode-side-by-side"));
        fireEvent.click(screen.getByTestId("split-screen-mode-quad"));

        await waitFor(() => {
            expect((screen.getByTestId("pane-input-c") as HTMLInputElement).value).toBe(
                "keep hidden"
            );
        });
    });

    test("unmounts hidden panes when hiddenPaneBehavior is unmount", async () => {
        const { SplitScreen } = await import("./index");

        await renderBasic(
            <SplitScreen
                defaultMode="quad"
                showModeControls
                hiddenPaneBehavior="unmount"
                panes={[
                    {
                        id: "a",
                        node: <StatefulPaneWithHandle id="a" />
                    },
                    {
                        id: "b",
                        node: <StatefulPaneWithHandle id="b" />
                    },
                    {
                        id: "c",
                        node: <StatefulPaneWithHandle id="c" />
                    },
                    {
                        id: "d",
                        node: <StatefulPaneWithHandle id="d" />
                    }
                ]}
            />
        );

        const input = screen.getByTestId("pane-input-c") as HTMLInputElement;
        fireEvent.change(input, { target: { value: "reset me" } });
        expect(input.value).toBe("reset me");

        fireEvent.click(screen.getByTestId("split-screen-mode-side-by-side"));
        fireEvent.click(screen.getByTestId("split-screen-mode-quad"));

        await waitFor(() => {
            expect((screen.getByTestId("pane-input-c") as HTMLInputElement).value).toBe(
                ""
            );
        });
    });

    test("renders no placeholder UI for empty sectors", async () => {
        const { SplitScreen } = await import("./index");

        await renderBasic(
            <SplitScreen
                panes={[{ id: "a", node: <div data-testid="pane-a-content">A</div> }]}
            />
        );

        expect(screen.getByTestId("split-screen-sector-right")).toBeInTheDocument();
        expect(screen.queryByText("Empty sector")).not.toBeInTheDocument();
    });

    test("useSplitScreenHandle throws outside SplitScreen pane content", async () => {
        const { useSplitScreenHandle } = await import("./index");

        const Outside = () => {
            useSplitScreenHandle("outside");
            return <div>outside</div>;
        };

        await expect(renderBasic(<Outside />, true)).rejects.toThrow(
            /useSplitScreenHandle must be used within <SplitScreen \/> pane content\./
        );
    });
});
