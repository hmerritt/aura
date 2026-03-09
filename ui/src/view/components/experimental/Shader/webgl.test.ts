import { describe, expect, test } from "vitest";

import { normalizeShaderGraphInput, resolveBufferPassOrder } from "./webgl";

const shader = {
	inline: `void mainImage(out vec4 fragColor, vec2 fragCoord) {
		fragColor = vec4(1.0);
	}`
} as const;

describe("Shader graph normalization", () => {
	test("normalizes inline input into an image-only graph", () => {
		const graph = normalizeShaderGraphInput({
			inline: shader.inline
		});

		expect(graph.image.id).toBe("image");
		expect(graph.image.shader).toEqual({ inline: shader.inline });
		expect(graph.image.channels.length).toBe(4);
		expect(graph.buffers).toEqual([]);
	});

	test("normalizes url input into an image-only graph", () => {
		const graph = normalizeShaderGraphInput({
			url: "https://example.com/shader.glsl"
		});

		expect(graph.image.shader).toEqual({ url: "https://example.com/shader.glsl" });
	});

	test("throws when a pass references an unknown pass input", () => {
		expect(() =>
			normalizeShaderGraphInput({
				graph: {
					buffers: [
						{
							id: "bufferA",
							shader,
							channels: [{ type: "pass", passId: "missing" }]
						}
					],
					image: { shader }
				}
			})
		).toThrow('references unknown pass "missing"');
	});

	test("throws when more than 4 channels are supplied", () => {
		expect(() =>
			normalizeShaderGraphInput({
				graph: {
					image: {
						shader,
						channels: [
							{ type: "texture", url: "a" },
							{ type: "texture", url: "b" },
							{ type: "texture", url: "c" },
							{ type: "texture", url: "d" },
							{ type: "texture", url: "e" }
						]
					}
				}
			})
		).toThrow("can only have 4 channels");
	});

	test("throws when input mode is ambiguous", () => {
		const ambiguousInput = {
			inline: shader.inline,
			url: "https://example.com/shader.glsl"
		};

		expect(() =>
			normalizeShaderGraphInput(
				ambiguousInput as unknown as Parameters<
					typeof normalizeShaderGraphInput
				>[0]
			)
		).toThrow("must define exactly one mode");
	});
});

describe("Shader buffer pass ordering", () => {
	test("resolves ordered dependencies", () => {
		const graph = normalizeShaderGraphInput({
			graph: {
				buffers: [
					{ id: "bufferA", shader },
					{
						id: "bufferB",
						shader,
						channels: [{ type: "pass", passId: "bufferA" }]
					},
					{
						id: "bufferC",
						shader,
						channels: [{ type: "pass", passId: "bufferB" }]
					}
				],
				image: {
					shader,
					channels: [{ type: "pass", passId: "bufferC" }]
				}
			}
		});

		expect(resolveBufferPassOrder(graph)).toEqual(["bufferA", "bufferB", "bufferC"]);
	});

	test("allows self-feedback references", () => {
		const graph = normalizeShaderGraphInput({
			graph: {
				buffers: [
					{
						id: "bufferA",
						shader,
						channels: [{ type: "pass", passId: "bufferA" }]
					}
				],
				image: {
					shader,
					channels: [{ type: "pass", passId: "bufferA" }]
				}
			}
		});

		expect(resolveBufferPassOrder(graph)).toEqual(["bufferA"]);
	});

	test("rejects cyclic cross-pass dependencies", () => {
		expect(() =>
			normalizeShaderGraphInput({
				graph: {
					buffers: [
						{
							id: "bufferA",
							shader,
							channels: [{ type: "pass", passId: "bufferB" }]
						},
						{
							id: "bufferB",
							shader,
							channels: [{ type: "pass", passId: "bufferA" }]
						}
					],
					image: { shader }
				}
			})
		).toThrow("cyclic pass dependencies");
	});
});
