import { updateSlice } from "state/index";

export const colorNext = () => {
	updateSlice("color", (color) => {
		color.current = color.colors[Math.floor(Math.random() * color.colors.length)];
	});
};
