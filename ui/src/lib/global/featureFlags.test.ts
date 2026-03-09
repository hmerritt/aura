// sort-imports-ignore
import { get } from "lib/type-guards";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import * as envImport from "./env";
import { feature } from "./featureFlags";

vi.mock("./env");

const initialEnv = {
	isProd: false,
	isDev: false,
	true: true,
	trueInt: 1,
	trueString: "truthy",
	trueStringInt: "1",
	false: false,
	falseInt: 0,
	falseString: "false",
	falsyString: "",
	falseNull: null,
	falseUndefined: undefined
};

describe("In Production Environment", () => {
	beforeEach(() => {
		vi.mocked(envImport).envGet.mockImplementation((key: any) => {
			return get({ ...initialEnv, isProd: true }, key);
		});
	});

	afterEach(() => {
		vi.resetAllMocks();
	});

	test("should return TRUE if flag is explicitly true", () => {
		const TEST_FLAG = "true" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(true);
		expect(vi.mocked(envImport).envGet).toHaveBeenCalledWith(TEST_FLAG);
	});

	test('should return TRUE if flag value is "1"', () => {
		const TEST_FLAG = "trueStringInt" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(true);
	});

	test("should return TRUE if flag value is any non-falsy string", () => {
		const TEST_FLAG = "trueString" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(true);
	});

	test("should return FALSE if flag value parses to false", () => {
		const TEST_FLAG = "falseString" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
	});

	test('should return FALSE if flag value is "0"', () => {
		const TEST_FLAG = "falseInt" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
	});

	test("should return FALSE if flag is not defined (envGet returns undefined)", () => {
		const TEST_FLAG = "falseUndefined" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
	});

	test("should return FALSE if flag is null (envGet returns null)", () => {
		const TEST_FLAG = "falseNull" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
	});

	test("should ignore alwaysShowOnDev option in production", () => {
		const TEST_FLAG = "falseNull" as any;
		expect(feature(TEST_FLAG, { alwaysShowOnDev: true })).toBe(false);
		expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
	});
});

describe("In Development Environment", () => {
	beforeEach(() => {
		vi.mocked(envImport).envGet.mockImplementation((key: any) => {
			return get({ ...initialEnv, isDev: true }, key);
		});
	});

	afterEach(() => {
		vi.resetAllMocks();
	});

	// --- Tests with alwaysShowOnDev: true ---

	test('should return TRUE by default if flag value is NOT explicitly "false" (alwaysShowOnDev=true)', () => {
		const TEST_FLAGS = [
			"true",
			"trueInt",
			"trueString",
			"trueStringInt",
			"falseInt",
			"falsyString",
			"falseNull",
			"falseUndefined"
		] as any[];

		for (const TEST_FLAG of TEST_FLAGS) {
			expect(feature(TEST_FLAG, { alwaysShowOnDev: true })).toBe(true);
		}
	});

	test("should return FALSE if flag value parses to exactly false (alwaysShowOnDev=true)", () => {
		const TEST_FLAGS = ["false", "falseString"] as any[];

		for (const TEST_FLAG of TEST_FLAGS) {
			expect(feature(TEST_FLAG, { alwaysShowOnDev: true })).toBe(false);
		}
	});

	// --- Tests with alwaysShowOnDev: false ---

	test("should return TRUE if flag parses to true when alwaysShowOnDev=false", () => {
		const TEST_FLAGS = ["true", "trueInt", "trueString", "trueStringInt"] as any[];

		for (const TEST_FLAG of TEST_FLAGS) {
			expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(true);
		}
	});

	test("should return FALSE if flag parses to false when alwaysShowOnDev=false", () => {
		const TEST_FLAGS = [
			"false",
			"falseInt",
			"falsyString",
			"falseNull",
			"falseUndefined"
		] as any[];

		for (const TEST_FLAG of TEST_FLAGS) {
			expect(feature(TEST_FLAG, { alwaysShowOnDev: false })).toBe(false);
		}
	});
});
