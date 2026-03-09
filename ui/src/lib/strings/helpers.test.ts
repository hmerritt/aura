import { describe, expect, test } from "vitest";

import { silenceLogs } from "tests/utils";

import {
	enforceLen,
	padChar,
	parseJSON,
	pluralize,
	trimPrefix,
	trimSuffix
} from "./helpers";

test("enforceLen", () => {
	const inputs: [string, number, boolean][] = [
		["Hello World!", 5, false],
		["Hello World!", 20, false],
		["", 5, false],
		["Hello World!", 5, true]
	];
	const outputs = ["Hello", "Hello World!", "", "Hello..."];

	for (let i = 0; i < inputs.length; i++) {
		const input = inputs[i];
		const result = enforceLen(...input);
		expect(result).toEqual(outputs[i]);
	}
});

test("padChar", () => {
	expect(padChar("", 5, "!")).toBe("!!!!!");
	expect(padChar("hello", 5, "!", false)).toBe("hello");
	expect(padChar("hello", 10, " ", false)).toBe("     hello");
	expect(padChar(12345, 10, "!", false)).toBe("!!!!!12345");

	// append: true
	expect(padChar("", 5, "!", true)).toBe("!!!!!");
	expect(padChar("hello", 5, "!", true)).toBe("hello");
	expect(padChar("hello", 10, " ", true)).toBe("hello     ");
	expect(padChar(12345, 10, "!", true)).toBe("12345!!!!!");
});

describe("parseJSON", () => {
	silenceLogs();

	test("parseJSON", () => {
		expect(parseJSON(`{"key":"some value"}`).key).toBe("some value");
		expect(parseJSON("[1]")).toStrictEqual([1]);

		expect(parseJSON(`{"key:not valid json"}`)).toBe(undefined);
		expect(parseJSON("[wow")).toBe(undefined);
	});
});

test("pluralize", () => {
	const inputs: [number, string, string?][] = [
		[1, "follower"],
		[1, "member"],
		[100, "post"],
		[1000, "repost"],
		[10000, "upvote"],
		[100000, "other"],
		[2, "man", "men"]
	];
	const outputs = [
		"follower",
		"member",
		"posts",
		"reposts",
		"upvotes",
		"others",
		"men"
	];

	for (let i = 0; i < inputs.length; i++) {
		const input = inputs[i];
		const output = pluralize(...input);
		expect(output).toEqual(outputs[i]);
	}
});

test("trimPrefix", () => {
	expect(trimPrefix("Hello World", "Hello W")).toBe("orld");
	expect(trimPrefix("!@#wow#(*&#/", "!@#w")).toBe("ow#(*&#/");

	expect(trimPrefix("Hello World", " World")).toBe("Hello World");
	expect(trimPrefix("!@#wow#(*&#/", "@#wo")).toBe("!@#wow#(*&#/");
});

test("trimSuffix", () => {
	expect(trimSuffix("Hello World", "o World")).toBe("Hell");
	expect(trimSuffix("!@#wow#(*&#/", "w#(*&#/")).toBe("!@#wo");

	expect(trimSuffix("Hello World", " Hello ")).toBe("Hello World");
	expect(trimSuffix("!@#wow#(*&#/", "(*&#")).toBe("!@#wow#(*&#/");
});
