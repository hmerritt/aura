import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { LogStore } from "./log";

describe("LogStore", () => {
	let logStore: LogStore;
	const mockTimestamp = 1712999999000; // A fixed point in time for testing

	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(mockTimestamp);

		logStore = new LogStore();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	describe("constructor", () => {
		it("should initialize with the correct default namespace", () => {
			expect(logStore.defaultNamespace).toBe("_log");
		});

		it("should initialize the logStore with the default namespace entry", () => {
			expect(logStore.logStore["_log"]).toBeDefined();
			expect(logStore.logStore["_log"]).toBeInstanceOf(Array);
		});

		it("should initialize the default entry with the current timestamp", () => {
			expect(logStore.logStore["_log"][0]).toBe(mockTimestamp);
		});

		it("should initialize the default entry with a count of 1", () => {
			expect(logStore.logStore["_log"][1]).toBe(1);
		});
	});

	describe("get()", () => {
		it("should return the entry for the default namespace when called without arguments", () => {
			expect(logStore.get()).toEqual([mockTimestamp, 1]);
		});

		it("should return the entry for the specified namespace", () => {
			expect(logStore.get("_log")).toEqual([mockTimestamp, 1]);
		});

		it("should return undefined for a non-existent namespace", () => {
			expect(logStore.get("nonexistent")).toBeUndefined();
		});

		it("should return the correct entry after setting a custom namespace", () => {
			const customTime = mockTimestamp + 1000;
			logStore.set("custom", customTime, 5);
			expect(logStore.get("custom")).toEqual([customTime, 5]);
		});
	});

	describe("getTime()", () => {
		it("should return the timestamp for the default namespace when called without arguments", () => {
			expect(logStore.getTime()).toBe(mockTimestamp);
		});

		it("should return the timestamp for the specified namespace", () => {
			expect(logStore.getTime("_log")).toBe(mockTimestamp);
		});

		it("should return the current timestamp for a non-existent namespace", () => {
			// Advance time slightly to ensure it returns the *new* Date.now()
			const futureTime = mockTimestamp + 5000;
			vi.setSystemTime(futureTime);
			expect(logStore.getTime("nonexistent")).toBe(futureTime);
		});

		it("should return the correct timestamp after setting a custom namespace", () => {
			const customTime = mockTimestamp - 10000; // Past time
			logStore.set("custom", customTime, 5);
			expect(logStore.getTime("custom")).toBe(customTime);
		});
	});

	describe("getCount()", () => {
		it("should return the count for the default namespace when called without arguments", () => {
			expect(logStore.getCount()).toBe(1);
		});

		it("should return the count for the specified namespace", () => {
			expect(logStore.getCount("_log")).toBe(1);
		});

		it("should return 1 for a non-existent namespace (as per implementation)", () => {
			expect(logStore.getCount("nonexistent")).toBe(1);
		});

		it("should return the correct count after setting a custom namespace", () => {
			logStore.set("custom", mockTimestamp, 99);
			expect(logStore.getCount("custom")).toBe(99);
		});
	});

	describe("set()", () => {
		it("should create a new entry with default time and count when only namespace is provided", () => {
			const newTime = mockTimestamp + 2000;
			vi.setSystemTime(newTime); // Simulate time passing before set
			const result = logStore.set("new_ns");

			expect(result).toEqual([newTime, 1]);
			expect(logStore.logStore["new_ns"]).toEqual([newTime, 1]);
			expect(logStore.get("new_ns")).toEqual([newTime, 1]);
		});

		it("should create/update an entry with the specified namespace, time, and count", () => {
			const customTime = mockTimestamp - 5000;
			const customCount = 10;
			const result = logStore.set("another_ns", customTime, customCount);

			expect(result).toEqual([customTime, customCount]);
			expect(logStore.logStore["another_ns"]).toEqual([customTime, customCount]);
			expect(logStore.get("another_ns")).toEqual([customTime, customCount]);
		});

		it("should update an existing entry (e.g., the default namespace)", () => {
			const updatedTime = mockTimestamp + 10000;
			const updatedCount = 5;
			const result = logStore.set("_log", updatedTime, updatedCount); // Update default

			expect(result).toEqual([updatedTime, updatedCount]);
			expect(logStore.logStore["_log"]).toEqual([updatedTime, updatedCount]);
			expect(logStore.get("_log")).toEqual([updatedTime, updatedCount]);
		});

		it("should update the entry for the default namespace when called without arguments", () => {
			const updatedTime = mockTimestamp + 15000;
			vi.setSystemTime(updatedTime);
			// set() defaults time=Date.now(), count=1
			const result = logStore.set(); // Should affect '_log'

			expect(result).toEqual([updatedTime, 1]);
			expect(logStore.logStore["_log"]).toEqual([updatedTime, 1]);
			expect(logStore.get("_log")).toEqual([updatedTime, 1]);
		});
	});

	describe("increment()", () => {
		it("should increment the count and update the timestamp for the default namespace", () => {
			const incrementTime = mockTimestamp + 3000;
			vi.setSystemTime(incrementTime);
			const result = logStore.increment(); // Increment '_log'

			expect(result).toEqual([incrementTime, 2]); // Initial count was 1
			expect(logStore.get("_log")).toEqual([incrementTime, 2]);
			expect(logStore.getCount("_log")).toBe(2);
			expect(logStore.getTime("_log")).toBe(incrementTime);
		});

		it("should increment the count and update the timestamp for a specified existing namespace", () => {
			const setupTime = mockTimestamp + 4000;
			const initialCount = 5;
			logStore.set("existing_ns", setupTime, initialCount);

			const incrementTime = mockTimestamp + 5000;
			vi.setSystemTime(incrementTime);
			const result = logStore.increment("existing_ns");

			expect(result).toEqual([incrementTime, initialCount + 1]);
			expect(logStore.get("existing_ns")).toEqual([incrementTime, 6]);
			expect(logStore.getCount("existing_ns")).toBe(6);
			expect(logStore.getTime("existing_ns")).toBe(incrementTime);
		});

		it("should create a new entry starting at count 2 for a non-existent namespace", () => {
			// This happens because getCount('nonexistent') returns 1
			const incrementTime = mockTimestamp + 6000;
			vi.setSystemTime(incrementTime);
			const result = logStore.increment("brand_new_ns");

			// Expected count is getCount('brand_new_ns') [returns 1] + 1 = 2
			expect(result).toEqual([incrementTime, 2]);
			expect(logStore.get("brand_new_ns")).toEqual([incrementTime, 2]);
			expect(logStore.getCount("brand_new_ns")).toBe(2);
			expect(logStore.getTime("brand_new_ns")).toBe(incrementTime);
		});
	});
});
