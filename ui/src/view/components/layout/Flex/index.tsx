import * as stylex from "@stylexjs/stylex";

import { type SxProp } from "lib/type-assertions";

export type FlexProps = React.JSX.IntrinsicElements["div"] &
	SxProp & {
		center?: boolean;
		grow?: boolean;
		row?: boolean;
		shrink?: boolean;
		wrap?: boolean;
		vc?: boolean;
		hc?: boolean;
	};

export const Flex = ({
	sx,
	center = false,
	grow = false,
	row = false,
	shrink = false,
	wrap = false,
	vc = false,
	hc = false,
	...props
}: FlexProps) => {
	return (
		<div
			{...props}
			{...stylex.props(
				flexStyles.flex,
				center && flexStyles.center,
				grow && flexStyles.grow,
				row && flexStyles.row,
				shrink && flexStyles.shrink,
				wrap && flexStyles.wrap,
				vc && flexStyles.vc,
				hc && flexStyles.hc,
				sx
			)}
		/>
	);
};

export const flexStyles = stylex.create({
	flex: {
		display: "flex",
		flexWrap: "nowrap",
		flexDirection: "column",
		minWidth: 0 // Fixes overflow issues
	},
	center: {
		alignItems: "center",
		justifyContent: "center"
	},
	row: {
		flexDirection: "row"
	},
	grow: {
		flexGrow: 1
	},
	shrink: {
		flexShrink: 1
	},
	vc: {
		alignItems: "center"
	},
	hc: {
		justifyContent: "center"
	},
	wrap: {
		flexWrap: "wrap"
	}
});
