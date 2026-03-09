import { expect, test, vi } from "vitest";

import { mutate } from "./mutate";

test("mutate", () => {
	const initialState = {
		int: -1,
		str: "",
		arr: [1, 2, 3],
		obj: { a: 0, b: 101 } as Record<string, number>
	};
	const nextState = mutate(initialState, (draft) => {
		draft.int += 101;
		draft.str = "hello";
		draft.arr.push(4);
		draft.arr.push(5);
		draft.arr.push(6);
		draft.obj.b = 1;
		draft.obj.c = 2;
	});

	expect(nextState.int).toBe(100);
	expect(nextState.str).toBe("hello");
	expect(nextState.arr).toEqual([1, 2, 3, 4, 5, 6]);
	expect(nextState.obj).toEqual({ a: 0, b: 1, c: 2 });
});

test("should return a new object reference", () => {
	const initialState = { count: 5 };
	const nextState = mutate(initialState, (draft) => {
		draft.count += 1;
	});
	expect(nextState).not.toBe(initialState); // Should be a new object
	expect(nextState).toEqual({ count: 6 }); // Values should match
});

test("should call callbacks with previous state, next state, and title", () => {
	const initialState = { count: 10 };
	const mockCallback = vi.fn();
	const options = {
		callbacks: [mockCallback],
		mutateTitle: "Test Mutate"
	};
	const expectedNextState = { count: 11 }; // Manually determine expected next state

	const nextState = mutate(
		initialState,
		(draft) => {
			draft.count++;
		},
		options
	);

	expect(nextState).toEqual(expectedNextState);
	expect(mockCallback).toHaveBeenCalledTimes(1);
	expect(mockCallback).toHaveBeenCalledWith(
		initialState, // prev state
		expectedNextState, // next state (as mutated)
		options.mutateTitle // title
	);
});

test("should handle multiple callbacks", () => {
	const initialState = { count: 10 };
	const mockCallback1 = vi.fn();
	const mockCallback2 = vi.fn();
	const options = {
		callbacks: [mockCallback1, mockCallback2]
	};
	mutate(
		initialState,
		(draft) => {
			draft.count++;
		},
		options
	);

	expect(mockCallback1).toHaveBeenCalledTimes(1);
	expect(mockCallback2).toHaveBeenCalledTimes(1);
	expect(mockCallback1).toHaveBeenCalledWith(initialState, { count: 11 }, undefined);
	expect(mockCallback2).toHaveBeenCalledWith(initialState, { count: 11 }, undefined);
});

test("should work without callbacks", () => {
	const initialState = { count: 5 };
	const nextState = mutate(initialState, (draft) => {
		draft.count += 1;
	});
	// No explicit callback expectation needed, just ensure it doesn't crash
	expect(nextState).toEqual({ count: 6 });
});

test("should handle mutations on nested objects NEW (shallow copy behavior)", () => {
	const initialState = { data: { value: 1 }, other: "test" };
	const nextState = mutate(initialState, (draft) => {
		draft.data.value = 2; // Mutate nested property
	});

	expect(nextState).not.toBe(initialState); // New outer object
	expect(nextState.data).toBe(initialState.data); // Shallow copy keeps nested object ref
	expect(initialState.data.value).toBe(2); // Nested mutation affects original
	expect(nextState.data.value).toBe(2);
	expect(nextState.other).toBe("test"); // Unchanged primitive
});
