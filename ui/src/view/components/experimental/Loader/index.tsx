import { type SxProp } from "lib/type-assertions";

import * as Loaders from "./subComponents";
import { type LoaderVariantProps } from "./subComponents/props";

export type LoaderType = keyof typeof Loaders;

export type LoaderProps = Omit<React.JSX.IntrinsicElements["div"], "style"> &
    SxProp & {
        type: LoaderType;
        /** Overall loader size (number values are interpreted as px) */
        size?: number | string;
        /** Speed multiplier (`2` is twice as fast as `1`) */
        speed?: number;
        /** Color applied to all dots */
        color?: string;
    };

const getDuration = (speed: number) => {
    if (!Number.isFinite(speed) || speed <= 0) return 900;
    return Math.min(10_000, Math.max(120, Math.round(900 / speed)));
};

export const Loader = ({
    type,
    size = "1.25rem",
    speed = 1,
    color = "currentColor",
    ...props
}: LoaderProps) => {
    const LoaderComponent = Loaders?.[type] as React.ComponentType<LoaderVariantProps>;

    return (
        <LoaderComponent
            {...props}
            size={size}
            durationMs={getDuration(speed)}
            color={color}
        />
    );
};
