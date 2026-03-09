import * as stylex from "@stylexjs/stylex";

import { type SxProp } from "lib/type-assertions";

export type GridProps = React.JSX.IntrinsicElements["div"] &
	SxProp & {
		maxWidth?: string | number;
		minWidth?: string | number;
		center?: boolean;
		gutter?: number;
	};

// Use `rem` if a number is passed, otherwise use the string as is.
const getUnit = (value: string | number) => {
	if (typeof value === "number") {
		return `${value}rem`;
	}

	return value;
};

export const Grid = ({
	center = false,
	children,
	gutter = 10,
	maxWidth = "1fr",
	minWidth = 100,
	sx,
	...props
}: GridProps) => {
	// Specify the minimum width of each item in the grid,
	// if an item is smaller than this, the grid will remove a column to make it fit.
	// prettier-ignore
	const gridTemplateColumns = `repeat(auto-fit, minmax(min(100%, ${getUnit(minWidth)}), ${getUnit(maxWidth)}))`;

	return (
		<div
			{...props}
			{...stylex.props(
				styles.grid({ gridTemplateColumns, gutter }),
				center && styles.center,
				sx
			)}
		>
			{children}
		</div>
	);
};

const styles = stylex.create({
	center: {
		justifyContent: "center"
	},
	grid: (s: { gridTemplateColumns: string; gutter: number }) => ({
		position: "relative",
		display: "grid",
		width: "100%",
		gridGap: s.gutter,
		gridTemplateColumns: `${s.gridTemplateColumns}`
	})
});
