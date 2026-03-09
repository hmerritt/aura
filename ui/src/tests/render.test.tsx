import { useRouterState } from "@tanstack/react-router";
import { screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { render, renderBasic } from "./render";

function RouterStateProbe() {
	const pathname = useRouterState({ select: (state) => state.location.pathname });
	return <div data-testid="router-pathname">{pathname}</div>;
}

describe("tests/render helper", () => {
	test("render mounts provided ui", async () => {
		await render(<div data-testid="child-content">hello</div>);
		expect(screen.getByTestId("child-content")).toHaveTextContent("hello");
	});

	test("render provides tanstack router context", async () => {
		await render(<RouterStateProbe />);
		expect(screen.getByTestId("router-pathname")).toHaveTextContent("/");
	});

	test("renderBasic still mounts content", async () => {
		await renderBasic(<div data-testid="basic-content">basic</div>);
		expect(screen.getByTestId("basic-content")).toHaveTextContent("basic");
	});
});
