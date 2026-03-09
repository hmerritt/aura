import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { ago } from "./time";

beforeEach(async () => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date(2001, 1, 1)); // Feb 01 2001
});

afterEach(async () => {
	vi.useRealTimers();
});

test("ago", () => {
	const inputs = [
		1671461038,
		"04 Dec 1995 00:12:00 GMT",
		new Date(),
		new Date().setMinutes(new Date().getMinutes() - 10),
		new Date().setHours(new Date().getHours() - 1),
		new Date().setDate(new Date().getDate() - 1),
		new Date().setMonth(new Date().getMonth() - 1)
	];
	const outputs = [
		new Date(1671461038).toLocaleDateString(),
		new Date("04 Dec 1995 00:12:00 GMT").toLocaleDateString(),
		"0s",
		"10m",
		"1h",
		"1d",
		"1mo"
	];

	for (let i = 0; i < inputs.length; i++) {
		const result = ago(inputs[i]);
		expect(result).toEqual(outputs[i]);
	}
});
