import { bench } from "vitest";

import { mutate } from "./mutate";

// --- State Definitions ---

const smallState = {
	int: -1,
	str: "",
	arr: [1, 2, 3],
	obj: { a: 0, b: 101 } as Record<string, number>
};

const mediumState: Record<string, any> = {};
for (let i = 0; i < 500; i++) {
	mediumState[`prop${i}`] = i;
	mediumState[`obj${i}`] = { value: i };
	mediumState[`arr${i}`] = [i, i + 1, i + 2];
}

const largeState: Record<string, any> = {};
for (let i = 0; i < 5000; i++) {
	largeState[`prop${i}`] = i;
}

const nestedState = {
	level1: {
		value: 1,
		level2: {
			value: 2,
			items: [10, 20, 30],
			level3: {
				value: 3,
				active: false
			}
		}
	},
	counter: 0
};

// --- Benchmark Cases ---

bench("[small] simple", () => {
	mutate(smallState, (draft) => {
		draft.int += 101;
		draft.str = "hello";
		draft.arr.push(4);
		draft.arr.push(5);
		draft.arr.push(6);
		draft.obj.b = 1;
		draft.obj.c = 2;
	});
});

bench("[small] minimal", () => {
	mutate(smallState, (draft) => {
		draft.int++;
	});
});

bench("[medium] simple", () => {
	mutate(mediumState, (draft) => {
		draft.prop0 = 999;
		draft.prop10 = 999;
		draft.obj5.value = 999; // Modify nested obj ref
		draft.arr8.push(999); // Modify nested array ref
	});
});

bench("[large] simple", () => {
	mutate(largeState, (draft) => {
		draft.prop0 = 999;
		draft.prop250 = 999;
		draft.prop499 = 999;
	});
});

bench("[nested] deep", () => {
	// Test modifying deeply nested properties
	// Note: This mutates the *original* nested objects/arrays due to shallow copy
	mutate(nestedState, (draft) => {
		draft.counter++;
		draft.level1.value++;
		draft.level1.level2.value++;
		draft.level1.level2.items.push(40);
		draft.level1.level2.level3.active = true;
	});
});
