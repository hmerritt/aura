import * as stylex from "@stylexjs/stylex";

import { Flex, type FlexProps, flexStyles } from "view/components";

interface StackProps extends FlexProps {
	spacing?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15;
}

export const Stack = ({
	sx,
	children,
	row = false,
	spacing = 0,
	...props
}: StackProps) => {
	return (
		<Flex
			{...props}
			row={row}
			sx={[
				flexStyles.flex,
				row ? stackStyles.directionRow : stackStyles.directionColumn,
				spacing === 0 && stackStyles.stack0,
				spacing === 1 && stackStyles.stack1,
				spacing === 2 && stackStyles.stack2,
				spacing === 3 && stackStyles.stack3,
				spacing === 4 && stackStyles.stack4,
				spacing === 5 && stackStyles.stack5,
				spacing === 6 && stackStyles.stack6,
				spacing === 7 && stackStyles.stack7,
				spacing === 8 && stackStyles.stack8,
				spacing === 9 && stackStyles.stack9,
				spacing === 10 && stackStyles.stack10,
				spacing === 11 && stackStyles.stack11,
				spacing === 12 && stackStyles.stack12,
				spacing === 13 && stackStyles.stack13,
				spacing === 14 && stackStyles.stack14,
				spacing === 15 && stackStyles.stack15,
				sx
			]}
		>
			{children}
		</Flex>
	);
};

export const stackStyles = stylex.create({
	directionColumn: {
		flexDirection: "column"
	},
	directionRow: {
		flexDirection: "row"
	},
	stack0: {
		gap: "0rem"
	},
	stack1: {
		gap: "0.2rem"
	},
	stack2: {
		gap: "0.5rem"
	},
	stack3: {
		gap: "1rem"
	},
	stack4: {
		gap: "1.5rem"
	},
	stack5: {
		gap: "2rem"
	},
	stack6: {
		gap: "2.5rem"
	},
	stack7: {
		gap: "3rem"
	},
	stack8: {
		gap: "3.5rem"
	},
	stack9: {
		gap: "4rem"
	},
	stack10: {
		gap: "4.5rem"
	},
	stack11: {
		gap: "5rem"
	},
	stack12: {
		gap: "5.5rem"
	},
	stack13: {
		gap: "6rem"
	},
	stack14: {
		gap: "6.5rem"
	},
	stack15: {
		gap: "7rem"
	}
});
