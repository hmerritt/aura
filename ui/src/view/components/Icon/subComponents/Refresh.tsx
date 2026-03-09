import * as stylex from "@stylexjs/stylex";
import { memo } from "react";

import { type IconSvgProps } from "./props";

export const Refresh = memo(({ sx, ...props }: IconSvgProps) => (
	<svg
		width="20"
		viewBox="0 0 24 20"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		{...props}
		{...stylex.props(sx)}
	>
		<path
			d="M23 2.00008V8.00008M23 8.00008H17M23 8.00008L18.36 3.64008C17.2853 2.56479 15.9556 1.77928 14.4952 1.35685C13.0348 0.934417 11.4911 0.888827 10.0083 1.22433C8.52547 1.55984 7.1518 2.26551 6.01547 3.27549C4.87913 4.28548 4.01717 5.56686 3.51 7.00008M1 18.0001V12.0001M1 12.0001H7M1 12.0001L5.64 16.3601C6.71475 17.4354 8.04437 18.2209 9.50481 18.6433C10.9652 19.0657 12.5089 19.1113 13.9917 18.7758C15.4745 18.4403 16.8482 17.7346 17.9845 16.7247C19.1209 15.7147 19.9828 14.4333 20.49 13.0001"
			stroke="currentColor"
			strokeWidth="2"
			strokeLinecap="round"
			strokeLinejoin="round"
		/>
	</svg>
));
