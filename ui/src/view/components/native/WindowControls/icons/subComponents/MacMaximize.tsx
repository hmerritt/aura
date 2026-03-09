import * as stylex from "@stylexjs/stylex";
import { memo } from "react";

import { type IconSvgProps } from "view/components/Icon/subComponents/props";

export const MacMaximize = memo(({ sx, ...props }: IconSvgProps) => (
	<svg
		xmlns="http://www.w3.org/2000/svg"
		viewBox="0 0 256 256"
		{...props}
		{...stylex.props(sx)}
	>
		<g stroke="none">
			<path
				stroke="none"
				fill="#0b7407"
				d="m155.55543,199.60027c1.94879,2.19849 3.43864,3.91809 5.52363,6.32464c-37.61765,0 -74.07987,0 -111.02196,0c0,-36.92317 0,-73.38108 0,-112.24213c35.87441,36.0108 70.45688,70.72468 105.49833,105.91749z"
			/>
			<path
				stroke="none"
				fill="#0b7407"
				d="m180.6005,135.96914c-28.59637,-28.54455 -56.73588,-56.6277 -86.06098,-85.89405c37.97663,0 74.34022,0 111.40336,0c0,36.26911 0,72.78582 0,111.20997c-8.80663,-8.79565 -16.84609,-16.82508 -25.34239,-25.31592z"
			/>
		</g>
	</svg>
));
