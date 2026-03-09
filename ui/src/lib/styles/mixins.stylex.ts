import * as stylex from "@stylexjs/stylex";

export const mixins = stylex.create({
	container: (size: string) => ({
		maxWidth: size,
		marginLeft: "auto",
		marginRight: "auto",
		transition: "all, 80ms, ease"
	}),
	textEllipsis: {
		overflow: "hidden",
		whiteSpace: "nowrap",
		textOverflow: "ellipsis"
	},
	gridColumnsRAM: (min: "150px", max: "1fr") => ({
		gridTemplateColumns: `repeat(auto-fit, minmax(${min}, ${max}))`
	}),
	gridColumns: (fitCount: 2, min: 0, max: "1fr") => ({
		gridTemplateColumns: `repeat(${fitCount}, minmax(${min}, ${max}))`
	}),
	scrollbar: (
		width: "1rem",
		height: "1rem",
		bgTrack: "transparent",
		bgThumb: "#b1b1b1",
		bgThumbHover: "#7e7e7e"
	) => ({
		"::-webkit-scrollbar": {
			width: width,
			height: height
		},
		"::-webkit-scrollbar-track": {
			borderRadius: "1000px",
			background: bgTrack
		},
		"::-webkit-scrollbar-thumb": {
			borderRadius: "1000px",
			background: bgThumb
		},
		// eslint-disable-next-line @stylexjs/valid-styles
		"::-webkit-scrollbar-thumb:hover": {
			background: bgThumbHover
		}
	})
});
