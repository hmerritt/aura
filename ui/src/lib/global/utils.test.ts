import { describe, expect, it, vi } from "vitest";

import { silenceLogs } from "tests/utils";

import { parseEnv, setGlobalValue } from "./utils";

silenceLogs();

describe("setGlobalValue", () => {
	it("should add a property with the correct key and value to the global object", () => {
		setGlobalValue("testKey", "testValue");
		expect(global).toHaveProperty("testKey", "testValue");
	});

	it("should define the property as non-writable", () => {
		const key = "readOnlyProp" as any;
		const initialValue = 123;
		setGlobalValue(key, initialValue);

		// Verify initial value
		expect(global).toHaveProperty(key, initialValue);

		try {
			(global as any)[key] = 456;
		} catch (_) {
			// In strict mode, this would throw TypeError. We expect it might fail silently otherwise.
		}

		// Verify the value hasn't changed
		expect(global).toHaveProperty(key, initialValue);

		// More robust check: Use getOwnPropertyDescriptor
		const descriptor = Object.getOwnPropertyDescriptor(global, key);
		expect(descriptor?.writable).toBe(false);
	});

	it("should throw TypeError when trying to write in strict mode", () => {
		const key = "strictWriteTest";
		setGlobalValue(key, "initial");

		// Use an immediately-invoked function expression (IIFE) with 'use strict'
		const attemptOverwrite = () => {
			"use strict";
			(global as any)[key] = "attempted change";
		};

		// Expect a TypeError when attempting to write to a non-writable property in strict mode
		expect(attemptOverwrite).toThrow(TypeError);
		// Verify value didn't change even after the throw attempt
		expect((global as any)[key]).toBe("initial");
	});

	it("should define the property as non-configurable", () => {
		const key = "nonConfigurableProp";
		const value = { id: 1 };
		setGlobalValue(key, value);

		// Attempt to delete (should fail silently in non-strict mode)
		try {
			delete (global as any)[key];
		} catch (_) {
			// In strict mode, this would throw TypeError
		}

		// Verify the property still exists
		expect(global).toHaveProperty(key);

		// More robust check: Use getOwnPropertyDescriptor
		const descriptor = Object.getOwnPropertyDescriptor(global, key);
		expect(descriptor?.configurable).toBe(false);
	});

	it("should throw TypeError when trying to delete in strict mode", () => {
		const key = "strictDeleteTest";
		setGlobalValue(key, "delete me");

		const attemptDelete = () => {
			"use strict";
			delete (global as any)[key];
		};

		// Expect a TypeError when attempting to delete a non-configurable property in strict mode
		expect(attemptDelete).toThrow(TypeError);
		expect(global).toHaveProperty(key);
	});

	it("should handle various value types correctly", () => {
		const testCases = [
			{ key: "numKey", value: 99 },
			{ key: "objKey", value: { nested: true } },
			{ key: "arrKey", value: [1, 2, 3] },
			{ key: "nullKey", value: null },
			{ key: "undefinedKey", value: undefined }
		];

		testCases.forEach(({ key, value }) => {
			setGlobalValue(key, value);
			const descriptor = Object.getOwnPropertyDescriptor(global, key);

			expect((global as any)[key]).toBe(value); // Use toBe for primitives/null/undefined
			if (typeof value === "object" && value !== null) {
				expect((global as any)[key]).toEqual(value);
			}
			expect(descriptor?.value).toBe(value);
			expect(descriptor?.writable).toBe(false);
			expect(descriptor?.configurable).toBe(false);
		});
	});

	it("should not allow overwriting even if called again with the same key", () => {
		const key = "overwriteTest";
		const firstValue = "Value A";
		const secondValue = "Value B";

		setGlobalValue(key, firstValue);
		expect((global as any)[key]).toBe(firstValue);

		// Call again with the same key but different value
		// Object.defineProperty will try to redefine, but fail because configurable=false, writable=false
		try {
			setGlobalValue(key, secondValue);
		} catch (_) {
			// This might throw a TypeError because you're trying to change attributes
			// of a non-configurable property, specifically the value on a non-writable one.
		}

		// The value should remain the first value set
		expect((global as any)[key]).toBe(firstValue);
	});
});

describe("parseEnv", () => {
	// --- Primitive String Parsing (isJson = false) ---
	describe("Primitive String Parsing (default)", () => {
		it('should parse "true" string to true boolean', () => {
			expect(parseEnv("true")).toBe(true);
		});

		it('should parse "false" string to false boolean', () => {
			expect(parseEnv("false")).toBe(false);
		});

		it('should parse "undefined" string to undefined value', () => {
			expect(parseEnv("undefined")).toBeUndefined();
		});

		it('should parse "null" string to null value', () => {
			expect(parseEnv("null")).toBeNull();
		});

		it('should be case-sensitive ("True" is not parsed)', () => {
			expect(parseEnv("True")).toBe("True");
		});

		it('should not trim whitespace (" true " is not parsed)', () => {
			expect(parseEnv(" true ")).toBe(" true ");
		});

		it("should return other strings as they are", () => {
			expect(parseEnv("hello world")).toBe("hello world");
			expect(parseEnv("12345")).toBe("12345");
			expect(parseEnv("")).toBe("");
		});
	});

	// --- Non-String Input Handling (isJson = false) ---
	describe("Non-String Input Handling (default)", () => {
		it("should return boolean true as is", () => {
			expect(parseEnv(true)).toBe(true);
		});

		it("should return boolean false as is", () => {
			expect(parseEnv(false)).toBe(false);
		});

		it("should return null as is", () => {
			expect(parseEnv(null)).toBeNull();
		});

		it("should return undefined as is", () => {
			expect(parseEnv(undefined)).toBeUndefined();
		});

		it("should return numbers as is", () => {
			expect(parseEnv(123)).toBe(123);
			expect(parseEnv(0)).toBe(0);
			expect(parseEnv(-1.5)).toBe(-1.5);
		});

		it("should return objects as is", () => {
			const obj = { a: 1 };
			expect(parseEnv(obj)).toBe(obj);
		});

		it("should return arrays as is", () => {
			const arr = [1, 2];
			expect(parseEnv(arr)).toBe(arr);
		});
	});

	// --- JSON Parsing (isJson = true) ---
	describe("JSON Parsing (isJson = true)", () => {
		it("should parse valid JSON number string to number", () => {
			expect(parseEnv("123.45", true)).toBe(123.45);
			expect(parseEnv("-10", true)).toBe(-10);
		});

		it("should parse valid JSON string value to string", () => {
			// Note: JSON strings need double quotes inside the string value
			expect(parseEnv('"hello"', true)).toBe("hello");
			expect(parseEnv('""', true)).toBe("");
		});

		it("should parse valid JSON object string to object", () => {
			const jsonString = '{ "user": "test", "id": 5 }';
			expect(parseEnv(jsonString, true)).toEqual({ user: "test", id: 5 });
		});

		it("should parse valid JSON array string to array", () => {
			const jsonString = '[true, null, "item", 99]';
			expect(parseEnv(jsonString, true)).toEqual([true, null, "item", 99]);
		});

		// --- Interaction with primitive checks ---
		it('should return boolean true for "true" string before attempting JSON parse', () => {
			expect(parseEnv("true", true)).toBe(true);
		});

		it('should return boolean false for "false" string before attempting JSON parse', () => {
			expect(parseEnv("false", true)).toBe(false);
		});

		it('should return null for "null" string before attempting JSON parse', () => {
			expect(parseEnv("null", true)).toBeNull();
		});

		it('should return undefined for "undefined" string before attempting JSON parse', () => {
			expect(parseEnv("undefined", true)).toBeUndefined();
		});

		// --- JSON Error Handling ---
		it("should return original value and call logn on invalid JSON string", () => {
			const invalidJson = "not json";
			expect(parseEnv(invalidJson, true)).toBe(invalidJson);
		});

		it("should return original value (empty string) and call logn on empty string", () => {
			const emptyString = "";
			// JSON.parse("") throws an error
			expect(parseEnv(emptyString, true)).toBe(emptyString);
		});

		it("should return original value (null) and call logn on null input", () => {
			// parseEnv(null, true) -> JSON.parse(null ?? "") -> JSON.parse("") -> throws
			expect(parseEnv(null, true)).toBeNull();
		});

		it("should return original value (undefined) and call logn on undefined input", () => {
			// parseEnv(undefined, true) -> JSON.parse(undefined ?? "") -> JSON.parse("") -> throws
			expect(parseEnv(undefined, true)).toBeUndefined();
		});
	});
});

describe("run", () => {
	it("should return [result, null] when the input promise resolves", async () => {
		const expectedResult = "Success Data";
		const inputPromise = Promise.resolve(expectedResult);

		const [result, error] = await run(inputPromise);

		expect(result).toBe(expectedResult);
		expect(error).toBeNull();
	});

	it("should return [result, null] when the input function returning a promise resolves", async () => {
		const expectedResult = { id: 1, value: "Test Object" };
		const promiseFn = vi.fn(() => Promise.resolve(expectedResult));

		const [result, error] = await run(promiseFn);

		expect(result).toEqual(expectedResult);
		expect(error).toBeNull();
		expect(promiseFn).toHaveBeenCalledTimes(1);
	});

	it("should return [null, error] when the input promise rejects", async () => {
		const expectedError = new Error("Something went wrong!");
		// Prevent Vitest/Node from complaining about unhandled rejections in the test itself
		const inputPromise = Promise.reject(expectedError).catch((err) => {
			throw err;
		});

		const [result, error] = await run(inputPromise);

		expect(result).toBeNull();
		expect(error).toBe(expectedError);
		expect(error).toBeInstanceOf(Error);
	});

	it("should return [null, error] when the input promise rejects with a non-Error value", async () => {
		const expectedError = "Just a string error";
		// Prevent Vitest/Node from complaining about unhandled rejections in the test itself
		const inputPromise = Promise.reject(expectedError).catch((err) => {
			throw err;
		});

		// Explicitly type the expected error type if it's not the default Error
		const [result, error] = await run<unknown, string>(inputPromise);

		expect(result).toBeNull();
		expect(error).toBe(expectedError);
	});

	it("should return [null, error] when the input function returning a promise rejects", async () => {
		const expectedError = new Error("Function failed!");
		const promiseFn = vi.fn(() => Promise.reject(expectedError));

		const [result, error] = await run(promiseFn);

		expect(result).toBeNull();
		expect(error).toBe(expectedError);
		expect(error).toBeInstanceOf(Error);
		expect(promiseFn).toHaveBeenCalledTimes(1);
	});

	it("should return [null, error] when the input function itself throws synchronously", async () => {
		const expectedError = new Error("Synchronous throw!");
		const throwingFn = vi.fn(() => {
			throw expectedError;
			// This line won't be reached, but satisfies TS if it expects a Promise return
			// return Promise.resolve('unreachable');
		});

		const [result, error] = await run(throwingFn);

		expect(result).toBeNull();
		expect(error).toBe(expectedError);
		expect(error).toBeInstanceOf(Error);
		expect(throwingFn).toHaveBeenCalledTimes(1);
	});

	it("should correctly handle different resolved types (e.g., number)", async () => {
		const expectedResult = 12345;
		const inputPromise = Promise.resolve(expectedResult);

		const [result, error] = await run(inputPromise);

		expect(result).toBe(expectedResult);
		expect(error).toBeNull();
	});

	it("should correctly handle different rejected types (e.g., custom object)", async () => {
		interface CustomError {
			code: number;
			message: string;
		}
		const expectedError: CustomError = { code: 404, message: "Not Found" };
		// Prevent Vitest/Node from complaining about unhandled rejections
		const inputPromise = Promise.reject(expectedError).catch((err) => {
			throw err;
		});

		// Explicitly type the expected error type
		const [result, error] = await run<unknown, CustomError>(inputPromise);

		expect(result).toBeNull();
		expect(error).toEqual(expectedError);
		expect(error?.code).toBe(404);
		expect(error?.message).toBe("Not Found");
	});
});
