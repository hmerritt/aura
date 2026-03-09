import * as stylex from "@stylexjs/stylex";

export const shadows = stylex.defineVars({
	zero: "0px 0px 0px rgba(0, 0, 0, 0)",
	/* Box shadows */
	box1: "0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24)",
	box2: "0 3px 6px rgba(0, 0, 0, 0.16), 0 3px 6px rgba(0, 0, 0, 0.23)",
	box3: "0 10px 20px rgba(0, 0, 0, 0.19), 0 6px 6px rgba(0, 0, 0, 0.23)",
	box4: "0 14px 28px rgba(0, 0, 0, 0.25), 0 10px 10px rgba(0, 0, 0, 0.22)",
	box5: "0 19px 38px rgba(0, 0, 0, 0.3), 0 15px 12px rgba(0, 0, 0, 0.22)",
	/* Text shadows */
	text1: "0px 0px 0x rgba(0, 0, 0, 0)",
	/* TEST VALUES: DO NOT DELETE */
	test2: "0 1px 3px rgba(0, 0, 0, 0.12), 0 1px 2px rgba(0, 0, 0, 0.24)"
});

export const shadowFn = stylex.create({
	boxBlock: (color: string) => ({
		boxShadow: `0.25px 0.25px 0 ${color}, 0.5px 0.5px 0 ${color}, 0.75px 0.75px 0 ${color},
			1px 1px 0 ${color}, 1.25px 1.25px 0 ${color}, 1.5px 1.5px 0 ${color},
			1.75px 1.75px 0 ${color}, 2px 2px 0 ${color}, 2.25px 2.25px 0 ${color},
			2.5px 2.5px 0 ${color}, 2.75px 2.75px 0 ${color}, 3px 3px 0 ${color},
			3.25px 3.25px 0 ${color}, 3.5px 3.5px 0 ${color}, 3.75px 3.75px 0 ${color},
			4px 4px 0 ${color}, 4.25px 4.25px 0 ${color}, 4.5px 4.5px 0 ${color}, 4.75px 4.75px 0 ${color},
			5px 5px 0 ${color}, 5.25px 5.25px 0 ${color}, 5.5px 5.5px 0 ${color}, 5.75px 5.75px 0 ${color},
			6px 6px 0 ${color}`
	}),
	textBlock: (color: string) => ({
		textShadow: `0.25px 0.25px 0 ${color}, 0.5px 0.5px 0 ${color}, 0.75px 0.75px 0 ${color},
			1px 1px 0 ${color}, 1.25px 1.25px 0 ${color}, 1.5px 1.5px 0 ${color},
			1.75px 1.75px 0 ${color}, 2px 2px 0 ${color}, 2.25px 2.25px 0 ${color},
			2.5px 2.5px 0 ${color}, 2.75px 2.75px 0 ${color}, 3px 3px 0 ${color},
			3.25px 3.25px 0 ${color}, 3.5px 3.5px 0 ${color}, 3.75px 3.75px 0 ${color},
			4px 4px 0 ${color}, 4.25px 4.25px 0 ${color}, 4.5px 4.5px 0 ${color}, 4.75px 4.75px 0 ${color},
			5px 5px 0 ${color}, 5.25px 5.25px 0 ${color}, 5.5px 5.5px 0 ${color}, 5.75px 5.75px 0 ${color},
			6px 6px 0 ${color}`
	})
});
