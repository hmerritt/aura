import * as stylex from "@stylexjs/stylex";

import { type SxProp } from "lib/type-assertions";

// @TODO: improve this

export const Container = ({
	sx,
	padding,
	width = "1320px",
	...props
}: React.JSX.IntrinsicElements["div"] &
	SxProp & {
		width?: string;
		padding?: string;
	}) => {
	return <div {...props} {...stylex.props(styles.container({ padding, width }), sx)} />;
};

const styles = stylex.create({
	container: (s) => ({
		position: "relative",
		width: "100%",
		maxWidth: s.width || "initial",
		marginLeft: "auto",
		marginRight: "auto",
		padding: {
			default: s.padding || "0 2rem",
			"@media screen and (max-width: 768px)": "0 1rem"
		}
	})
});
