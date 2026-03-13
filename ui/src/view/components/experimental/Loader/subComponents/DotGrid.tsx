import * as stylex from "@stylexjs/stylex";
import { useEffect, useState } from "react";

import { type LoaderVariantProps } from "./props";

type DotStyle =
    | "dot0"
    | "dot1"
    | "dot2"
    | "dot3"
    | "dot4"
    | "dot5"
    | "dot6"
    | "dot7"
    | "dot8";

type MotionFrame = {
    opacity: number;
    positions: [DotStyle, DotStyle, DotStyle];
    phase: "snake" | "converge" | "fade" | "pause";
};

const dotOrder: DotStyle[] = [
    "dot0",
    "dot1",
    "dot2",
    "dot3",
    "dot4",
    "dot5",
    "dot6",
    "dot7",
    "dot8"
];

const snakePath: DotStyle[] = [
    "dot0",
    "dot1",
    "dot2",
    "dot5",
    "dot8",
    "dot7",
    "dot6",
    "dot3",
    "dot4"
];
const center: DotStyle = "dot4";

const snakeFrames: MotionFrame[] = snakePath.map((_, index) => {
    const head = snakePath[Math.max(0, index)];
    const mid = snakePath[Math.max(0, index - 1)];
    const tail = snakePath[Math.max(0, index - 2)];
    return {
        opacity: 1,
        phase: "snake",
        positions: [head, mid, tail]
    };
});

const cycleFrames: MotionFrame[] = [
    ...snakeFrames,
    {
        opacity: 1,
        phase: "converge",
        positions: [center, center, "dot3"]
    },
    {
        opacity: 1,
        phase: "converge",
        positions: [center, center, center]
    },
    {
        opacity: 0.66,
        phase: "fade",
        positions: [center, center, center]
    },
    {
        opacity: 0.33,
        phase: "fade",
        positions: [center, center, center]
    },
    {
        opacity: 0,
        phase: "fade",
        positions: [center, center, center]
    },
    {
        opacity: 0,
        phase: "pause",
        positions: [center, center, center]
    }
];

const toCssSize = (value: number | string) =>
    typeof value === "number" ? `${value}px` : value;

const toDotSize = (value: number | string) => {
    if (typeof value === "number") return `${Math.max(2, value * 0.16)}px`;
    return `max(2px, calc(${value} * 0.16))`;
};

export const DotGrid = ({
    sx,
    size,
    durationMs,
    color,
    ...props
}: LoaderVariantProps) => {
    const [frameIndex, setFrameIndex] = useState(0);
    const activeFrame = cycleFrames[frameIndex];

    useEffect(() => {
        const beatsPerForwardCycle = snakePath.length + 1;
        const ms = Math.max(48, Math.round(durationMs / beatsPerForwardCycle));

        const intervalId = window.setInterval(() => {
            setFrameIndex((prev) => (prev + 1) % cycleFrames.length);
        }, ms);

        return () => {
            window.clearInterval(intervalId);
        };
    }, [durationMs]);

    const trailPositionKeys = activeFrame.positions;
    const trailPositionStyles = trailPositionKeys.map(
        (position) => positionStyles[position]
    );
    const loaderSize = toCssSize(size);
    const dotSize = toDotSize(size);
    const dotSizeStyle = styles.dotSize(dotSize);

    return (
        <div
            {...props}
            {...stylex.props(
                styles.root,
                styles.rootSize(loaderSize),
                styles.rootColor(color),
                sx
            )}
            data-loader-type="dotgrid"
            data-loader-duration-ms={durationMs}
            data-loader-color={color}
        >
            {dotOrder.map((dotStyle) => (
                <span
                    key={`slot-${dotStyle}`}
                    aria-hidden
                    data-loader-dot="slot"
                    {...stylex.props(styles.dot, dotSizeStyle, positionStyles[dotStyle])}
                />
            ))}
            {trailPositionStyles.map((trailPositionStyle, i) => (
                <span
                    key={`trail-${i}`}
                    aria-hidden
                    data-loader-dot="trail"
                    data-loader-opacity={activeFrame.opacity}
                    data-loader-phase={activeFrame.phase}
                    data-loader-position={trailPositionKeys[i]}
                    {...stylex.props(
                        styles.dot,
                        dotSizeStyle,
                        styles.trailDot,
                        trailPositionStyle,
                        styles.trailOpacity(activeFrame.opacity)
                    )}
                />
            ))}
        </div>
    );
};
const styles = stylex.create({
    root: {
        display: "inline-block",
        minHeight: "12px",
        minWidth: "12px",
        position: "relative"
    },
    rootSize: (loaderSize: string) => ({ height: loaderSize, width: loaderSize }),
    rootColor: (loaderColor: string) => ({ color: loaderColor }),
    dot: {
        backgroundColor: "currentColor",
        borderRadius: "9999px",
        left: "0%",
        opacity: 0.22,
        position: "absolute",
        top: "0%",
        transform: "translate(-50%, -50%)"
    },
    dotSize: (loaderDotSize: string) => ({
        height: loaderDotSize,
        width: loaderDotSize
    }),
    trailDot: {
        opacity: 1,
        willChange: "left, top"
    },
    trailOpacity: (opacity: number) => ({ opacity })
});

const positionStyles = stylex.create({
    dot0: { left: "0%", top: "0%" },
    dot1: { left: "50%", top: "0%" },
    dot2: { left: "100%", top: "0%" },
    dot3: { left: "0%", top: "50%" },
    dot4: { left: "50%", top: "50%" },
    dot5: { left: "100%", top: "50%" },
    dot6: { left: "0%", top: "100%" },
    dot7: { left: "50%", top: "100%" },
    dot8: { left: "100%", top: "100%" }
});
