import { beforeEach, describe, expect, it, vi } from "vitest";

import { PersistStorage, StoreWithPersist } from "./persist";

// Mock scheduler to just run immediately for tests
// Must match the import path used in persist.ts
vi.mock("lib/scheduler", () => ({
	createBackgroundScheduler: (fn: any) => {
		return (...args: any[]) => {
			return fn(...args);
		};
	}
}));

// Mock store setup
interface TestState {
	count: number;
	text: string;
}

describe("StoreWithPersist", () => {
	let mockStorage: Map<string, string>;
	let persistStorage: PersistStorage<Partial<TestState>>;

	beforeEach(() => {
		mockStorage = new Map();

		persistStorage = {
			getItem: vi.fn((name) => {
				const val = mockStorage.get(name);
				return val ? JSON.parse(val) : null;
			}),
			setItem: vi.fn((name, value) => {
				mockStorage.set(name, JSON.stringify(value));
			}),
			removeItem: vi.fn((name) => {
				mockStorage.delete(name);
			})
		};

		vi.clearAllMocks();
	});

	it("should initialize with default options", () => {
		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "hello" },
			{ name: "test-store", storage: persistStorage }
		);

		expect(store.state).toEqual({ count: 0, text: "hello" });
		expect(store.persist.name).toBe("test-store");
	});

	it("should persist state changes", async () => {
		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "hello" },
			{ name: "test-store", storage: persistStorage }
		);

		store.setState((prev) => ({ ...prev, count: 1 }));

		// Wait for next tick/promise resolution
		await new Promise((r) => setTimeout(r, 0));

		expect(persistStorage.setItem).toHaveBeenCalled();
		const val = mockStorage.get("test-store");
		expect(val).toBeDefined();
		expect(val).toContain('"count":1');
	});

	it("should hydrate from storage", async () => {
		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 5, text: "loaded" }, version: 0 })
		);

		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{ name: "test-store", storage: persistStorage }
		);

		await new Promise((r) => setTimeout(r, 0));

		expect(store.state).toEqual({ count: 5, text: "loaded" });
		expect(store.persist.hasHydrated()).toBe(true);
	});

	it("should handle partial updates (merge)", async () => {
		// Only 'count' is in storage
		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 10 }, version: 0 })
		);

		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "preserved" },
			{ name: "test-store", storage: persistStorage }
		);

		await new Promise((r) => setTimeout(r, 0));

		expect(store.state).toEqual({ count: 10, text: "preserved" });
	});

	it("should use partialize option", async () => {
		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "secret" },
			{
				name: "test-store",
				storage: persistStorage,
				partialize: (state) => ({ count: state.count }) as any // Only persist count
			}
		);

		store.setState((prev) => ({ ...prev, count: 2, text: "visible" }));
		await new Promise((r) => setTimeout(r, 0));

		const val = mockStorage.get("test-store");
		expect(val).toBeDefined();
		const stored = JSON.parse(val || "{}");
		expect(stored.state).toEqual({ count: 2 });
		expect(stored.state.text).toBeUndefined();
	});

	it("should handle version migration", async () => {
		// Old version 0 in storage
		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 5 }, version: 0 })
		);

		const migrate = vi.fn((oldState: any, version: number) => {
			if (version === 0) {
				return { count: oldState.count * 2, text: "migrated" };
			}
			return oldState;
		});

		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{
				name: "test-store",
				storage: persistStorage,
				version: 1,
				migrate
			}
		);

		await new Promise((r) => setTimeout(r, 0));

		expect(migrate).toHaveBeenCalled();
		expect(store.state).toEqual({ count: 10, text: "migrated" });

		// Should have updated storage with new version
		const val = mockStorage.get("test-store");
		expect(val).toBeDefined();
		const stored = JSON.parse(val || "{}");
		expect(stored.version).toBe(1);
		expect(stored.state).toEqual({ count: 10, text: "migrated" });
	});

	it("should call onRehydrateStorage", async () => {
		const afterHydrate = vi.fn();
		const onRehydrate = vi.fn(() => afterHydrate);

		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 5 }, version: 0 })
		);

		new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{
				name: "test-store",
				storage: persistStorage,
				onRehydrateStorage: onRehydrate
			}
		);

		await new Promise((r) => setTimeout(r, 0));

		expect(onRehydrate).toHaveBeenCalled();
		expect(afterHydrate).toHaveBeenCalled();
	});

	it("should respect skipHydration and allow manual rehydrate", async () => {
		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 5 }, version: 0 })
		);

		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{
				name: "test-store",
				storage: persistStorage,
				skipHydration: true
			}
		);

		await new Promise((r) => setTimeout(r, 0));
		expect(store.state.count).toBe(0); // Not hydrated yet

		if (store.persist.rehydrate) {
			await store.persist.rehydrate();
		}
		expect(store.state.count).toBe(5);
	});

	it("should clear storage", async () => {
		// Use valid JSON to avoid parse errors in internal hydration
		mockStorage.set(
			"test-store",
			JSON.stringify({ state: { count: 99 }, version: 0 })
		);

		const store = new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{ name: "test-store", storage: persistStorage }
		);

		store.persist.clearStorage();
		expect(mockStorage.has("test-store")).toBe(false);
	});

	it("should log when debug is true", async () => {
		// Ensure logn exists safely
		const logSrc = global.logn || { debug: vi.fn() };
		const debugSpy = vi.spyOn(logSrc as any, "debug");

		new StoreWithPersist<TestState>(
			{ count: 0, text: "init" },
			{
				name: "test-store-debug",
				storage: persistStorage,
				debug: true
			}
		);

		await new Promise((r) => setTimeout(r, 0));
		expect(debugSpy).toHaveBeenCalled();
	});
});
