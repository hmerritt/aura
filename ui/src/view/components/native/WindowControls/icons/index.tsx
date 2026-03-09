import { Ref } from "react";

import { type IconSvgProps } from "view/components/Icon/subComponents/props";

import * as WindowIcons from "./subComponents";

// Infer the type of IconMappings, then extract the keys from the type it infers
export type WindowIconsAvailable = keyof typeof WindowIcons;

type WindowIconProps = IconSvgProps & {
	name: WindowIconsAvailable;
	ref?: Ref<SVGSVGElement>;
};

export const WindowIcon = ({ name, ...svgProps }: WindowIconProps) => {
	const IconComponent = WindowIcons?.[name];
	return <IconComponent {...svgProps} />;
};
