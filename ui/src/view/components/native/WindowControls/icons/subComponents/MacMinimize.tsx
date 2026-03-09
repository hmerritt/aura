import * as stylex from "@stylexjs/stylex";
import { memo } from "react";

import { type IconSvgProps } from "view/components/Icon/subComponents/props";

export const MacMinimize = memo(({ sx, ...props }: IconSvgProps) => (
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 256 256"
		{...props}
		{...stylex.props(sx)}
	>
		<path
			stroke="none"
			fill="#975914"
			d="m232.77255,126.65765c-2.10552,12.53022 -6.15284,16.35554 -17.9543,16.37766c-57.76491,0.10832 -115.53019,0.09873 -173.29514,0.00805c-11.50925,-0.01807 -18.5844,-6.26587 -18.28662,-15.46683c0.28879,-8.92414 7.05706,-14.61102 18.2322,-14.62784c57.76496,-0.08705 115.53015,-0.01802 173.29519,-0.06263c9.27762,-0.00715 15.77765,3.49535 18.00868,13.7716z"
		/>
	</svg>
));
