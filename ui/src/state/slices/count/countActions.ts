import { updateSlice } from "state/index";

export const countIncrement = (incrementAmount = 0.1) => {
	updateSlice("count", (count) => {
		count.current = Number(
			(Number(count.current) + Number(incrementAmount)).toFixed(2)
		);
	});
};

export const countReset = () => {
	updateSlice("count", (count) => {
		count.current = 0;
	});
};
