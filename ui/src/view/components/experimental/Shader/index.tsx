import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef } from "react";

import { type SxProp } from "lib/type-assertions";

import {
	type ShaderInput,
	type ShaderState,
	createShaderState,
	normalizeShaderGraphInput,
	setup,
	teardown
} from "./webgl";

type ShaderPropsBase = React.JSX.IntrinsicElements["canvas"] & SxProp;

export type ShaderProps = ShaderPropsBase & { input: ShaderInput };

/**
 * Shader component
 *
 * Renders an image pass from inline GLSL/URL or a full shader graph with buffer passes.
 */
export const Shader = ({ input, sx, ...canvasProps }: ShaderProps) => {
	const canvas = useRef<HTMLCanvasElement>(null);
	const s = useRef<ShaderState>(createShaderState());
	const setupConfigKey = JSON.stringify(input);

	useEffect(() => {
		if (!canvas.current || env.isTest) return;

		const setupInput = JSON.parse(setupConfigKey) as ShaderInput;

		let normalizedGraph;
		try {
			normalizedGraph = normalizeShaderGraphInput(setupInput);
		} catch (error) {
			logn.error("shader", "Invalid shader configuration.", error);
			return;
		}

		const shaderState = createShaderState();
		s.current = shaderState;
		void setup(normalizedGraph, shaderState, canvas.current);

		return () => {
			teardown(shaderState);
		};
	}, [setupConfigKey]);

	return <canvas {...canvasProps} ref={canvas} {...stylex.props(sx)} />;
};
