import * as stylex from "@stylexjs/stylex";
import { useCallback, useEffect, useRef, useState } from "react";

import { type SxProp } from "lib/type-assertions";

export type FrostedGlassProps = React.JSX.IntrinsicElements["div"] &
	SxProp & {
		/** Pane direction */
		paneDirection?: "row" | "column";
		/** Target maximum width of each pane (only used when `paneDirection` is set to `row`) */
		paneMaxWidth?: number;
		/** Target maximum height of each pane (only used when `paneDirection` is set to `column`) */
		paneMaxHeight?: number;
		/** Initial number of panes shown before first render (pane count is re-calculated after component mounts) */
		paneInitialCount?: number;
		/** Recalculate pane count on window resize (responsive, but may impact performance) */
		reactToWindowResize?: boolean;
	};

/**
 * Frosted glass effect.
 *
 * Items behind the glass will be blurred (on the other side of a translucent glass pane).
 *
 * Inspired by Anders Tornblad's codepen: https://jsfiddle.net/atornblad/35orypsL/1/
 */
export const FrostedGlass: React.FC<FrostedGlassProps> = ({
	sx,
	children,
	paneDirection = "row",
	paneMaxWidth = 25,
	paneMaxHeight = 25,
	paneInitialCount = 5,
	reactToWindowResize = true,
	...divProps
}) => {
	const $div = useRef<HTMLDivElement>(null);

	const [paneCount, setPaneCount] = useState(paneInitialCount);

	const calculatePaneCount = useCallback(() => {
		if (!$div.current) return;

		const paneCalc =
			paneDirection === "row"
				? $div.current?.clientWidth / paneMaxWidth
				: $div.current?.clientHeight / paneMaxHeight;

		setPaneCount(Math.floor(paneCalc) || paneInitialCount);
	}, [paneDirection, paneInitialCount, paneMaxHeight, paneMaxWidth]);

	useEffect(() => {
		calculatePaneCount();

		if (reactToWindowResize) {
			window.addEventListener("resize", calculatePaneCount);
		}

		return () => {
			window.removeEventListener("resize", calculatePaneCount);
		};
	}, [reactToWindowResize, calculatePaneCount]);

	return (
		<div ref={$div} {...divProps} {...stylex.props(styles.frostedGlass, sx)}>
			{children}

			<div
				{...stylex.props(
					styles.paneContainer,
					paneDirection === "row" && styles.row,
					paneDirection === "column" && styles.column
				)}
			>
				{[...Array(paneCount).keys()].map((i) => (
					<div
						key={i}
						{...stylex.props(
							styles.pane,
							paneDirection === "row" && styles.fullHeight,
							paneDirection === "column" && styles.fullWidth
						)}
					/>
				))}
			</div>
		</div>
	);
};

const styles = stylex.create({
	column: {
		flexDirection: "column"
	},
	frostedGlass: {
		position: "relative",
		zIndex: 1
	},
	fullHeight: {
		height: "100%"
	},
	fullWidth: {
		width: "100%"
	},
	pane: {
		backdropFilter: "blur(8px)",
		backgroundColor: `linear-gradient(
			to right,
			rgba(255, 255, 255, 0.2),
			rgba(255, 255, 255, 0.1)
		)`,
		flex: "1"
	},
	paneContainer: {
		alignItems: "stretch",
		display: "flex",
		height: "100%",
		inset: 0,
		position: "absolute",
		width: "100%",
		zIndex: -2
	},
	row: {
		flexDirection: "row"
	}
});
