import { Ref } from "react";

import * as Icons from "./subComponents";
import { IconSvgProps } from "./subComponents/props";

// Infer the type of IconMappings, then extract the keys from the type it infers
export type IconsAvailable = keyof typeof Icons;

type IconProps = IconSvgProps & {
	name: IconsAvailable;
	animate?: string; // Try to make a class in `keyframes.scss` instead of using this prop
	ref?: Ref<SVGSVGElement>;
};

export const Icon = ({ name, animate, style = {}, ...svgProps }: IconProps) => {
	const styles = animate ? { animation: animate, ...style } : style;
	const IconComponent = Icons?.[name];
	return <IconComponent {...svgProps} style={styles} />;
};
