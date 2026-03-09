import { VERTEX_SHADER_SOURCE, createFragmentShaderSource } from "./glsl";

const CHANNEL_COUNT = 4;
const IMAGE_PASS_ID = "image";
const POSITION_ATTRIBUTE_LOCATION = 0;

export type ShaderPassSource =
	| {
			inline: string;
			url?: never;
	  }
	| {
			url: string;
			inline?: never;
	  };

export type ShaderChannelInput =
	| {
			type: "texture";
			url: string;
	  }
	| {
			type: "pass";
			passId: string;
	  };

export type ShaderPassSpec = {
	id: string;
	shader: ShaderPassSource;
	channels?: ReadonlyArray<ShaderChannelInput | null | undefined>;
};

export type ShaderGraph = {
	image: {
		shader: ShaderPassSource;
		channels?: ReadonlyArray<ShaderChannelInput | null | undefined>;
	};
	buffers?: ReadonlyArray<ShaderPassSpec>;
};

export type ShaderInput =
	| {
			/** GLSL shader code to inject into the fragment shader template */
			inline: string;
			url?: never;
			graph?: never;
	  }
	| {
			/** URL to fetch GLSL shader code */
			url: string;
			inline?: never;
			graph?: never;
	  }
	| {
			/** Shader graph definition */
			graph: ShaderGraph;
			inline?: never;
			url?: never;
	  };

export type NormalizedShaderPass = {
	id: string;
	shader: ShaderPassSource;
	channels: (ShaderChannelInput | null)[];
};

export type NormalizedShaderGraph = {
	image: NormalizedShaderPass;
	buffers: NormalizedShaderPass[];
};

type UniformLocations = {
	iResolution: WebGLUniformLocation | null;
	iTime: WebGLUniformLocation | null;
	iTimeDelta: WebGLUniformLocation | null;
	iFrame: WebGLUniformLocation | null;
	iMouse: WebGLUniformLocation | null;
	iDate: WebGLUniformLocation | null;
	iFrameRate: WebGLUniformLocation | null;
	iChannels: (WebGLUniformLocation | null)[];
};

type RenderTarget = {
	framebuffer: WebGLFramebuffer;
	texture: WebGLTexture;
};

type CompiledPass = {
	id: string;
	program: WebGLProgram;
	uniformLocations: UniformLocations;
	channels: (ShaderChannelInput | null)[];
};

type BufferPassRuntime = CompiledPass & {
	readTarget: RenderTarget;
	writeTarget: RenderTarget;
};

export type ShaderState = {
	gl: WebGL2RenderingContext | null;
	canvas: HTMLCanvasElement | null;
	startTime: number;
	frameTime: number;
	frameCount: number;
	rafId: number | null;
	isDisposed: boolean;
	vao: WebGLVertexArrayObject | null;
	positionBuffer: WebGLBuffer | null;
	fallbackTexture: WebGLTexture | null;
	loadedTextures: Map<string, WebGLTexture>;
	bufferPassesById: Map<string, BufferPassRuntime>;
	bufferPassOrder: string[];
	imagePass: CompiledPass | null;
	programs: WebGLProgram[];
	framebuffers: WebGLFramebuffer[];
	renderTextures: WebGLTexture[];
};

export const createShaderState = (): ShaderState => {
	return {
		gl: null,
		canvas: null,
		startTime: 0,
		frameTime: 0,
		frameCount: 0,
		rafId: null,
		isDisposed: false,
		vao: null,
		positionBuffer: null,
		fallbackTexture: null,
		loadedTextures: new Map(),
		bufferPassesById: new Map(),
		bufferPassOrder: [],
		imagePass: null,
		programs: [],
		framebuffers: [],
		renderTextures: []
	};
};

/**
 * Fetches GLSL shader code from a URL (does not check GLSL validity).
 *
 * @example
 * fetchShader("https://samples.threepipe.org/shaders/tunnel-cylinders.glsl")
 */
export const fetchShader = async (url: string): Promise<string> => {
	try {
		const response = await fetch(url);
		if (!response.ok) {
			throw new Error(`HTTP error! status: ${response.status}`);
		}
		return await response.text();
	} catch (error) {
		logn.error("shader", "Failed to fetch shader:", error);
		return "";
	}
};

const normalizeChannels = (
	channels?: ReadonlyArray<ShaderChannelInput | null | undefined>
): (ShaderChannelInput | null)[] => {
	if (!channels) {
		return Array.from({ length: CHANNEL_COUNT }, () => null);
	}

	if (channels.length > CHANNEL_COUNT) {
		throw new Error(
			`Shader pass can only have ${CHANNEL_COUNT} channels. Received ${channels.length}.`
		);
	}

	const normalized: (ShaderChannelInput | null)[] = Array.from(
		{ length: CHANNEL_COUNT },
		() => null
	);
	for (let i = 0; i < channels.length; i++) {
		const channel = channels[i];
		if (!channel) {
			continue;
		}
		normalized[i] = channel;
	}

	return normalized;
};

const normalizeFromSource = (source: ShaderPassSource): NormalizedShaderGraph => {
	return {
		image: {
			id: IMAGE_PASS_ID,
			shader: source,
			channels: normalizeChannels()
		},
		buffers: []
	};
};

const normalizeFromGraph = (graph: ShaderGraph): NormalizedShaderGraph => {
	const buffers: NormalizedShaderPass[] = [];
	const bufferIds = new Set<string>();

	for (const buffer of graph.buffers ?? []) {
		const id = buffer.id.trim();
		if (!id) {
			throw new Error("Shader buffer id cannot be empty.");
		}
		if (id === IMAGE_PASS_ID) {
			throw new Error(`Shader buffer id "${IMAGE_PASS_ID}" is reserved.`);
		}
		if (bufferIds.has(id)) {
			throw new Error(`Shader buffer id "${id}" is duplicated.`);
		}
		bufferIds.add(id);
		buffers.push({
			id,
			shader: buffer.shader,
			channels: normalizeChannels(buffer.channels)
		});
	}

	const normalized: NormalizedShaderGraph = {
		image: {
			id: IMAGE_PASS_ID,
			shader: graph.image.shader,
			channels: normalizeChannels(graph.image.channels)
		},
		buffers
	};

	for (const pass of [normalized.image, ...normalized.buffers]) {
		for (const channel of pass.channels) {
			if (!channel || channel.type !== "pass") {
				continue;
			}

			if (channel.passId === IMAGE_PASS_ID) {
				throw new Error("Pass channels cannot reference image output.");
			}

			if (!bufferIds.has(channel.passId)) {
				throw new Error(
					`Shader pass "${pass.id}" references unknown pass "${channel.passId}".`
				);
			}
		}
	}

	resolveBufferPassOrder(normalized);
	return normalized;
};

export const normalizeShaderGraphInput = (input: ShaderInput): NormalizedShaderGraph => {
	const hasInline = "inline" in input && typeof input.inline === "string";
	const hasUrl = "url" in input && typeof input.url === "string";
	const hasGraph =
		"graph" in input && typeof input.graph === "object" && input.graph !== null;
	const modeCount = Number(hasInline) + Number(hasUrl) + Number(hasGraph);

	if (modeCount !== 1) {
		throw new Error(
			"Shader input must define exactly one mode: `inline`, `url`, or `graph`."
		);
	}

	if (hasInline) {
		return normalizeFromSource({ inline: input.inline });
	}

	if (hasUrl) {
		return normalizeFromSource({ url: input.url });
	}

	if (hasGraph) {
		return normalizeFromGraph(input.graph);
	}

	throw new Error("Shader input mode could not be resolved.");
};

/**
 * Resolves a valid execution order for buffer passes.
 *
 * Self-references are allowed and treated as previous-frame feedback.
 */
export const resolveBufferPassOrder = (graph: NormalizedShaderGraph): string[] => {
	const orderHint = graph.buffers.map((buffer) => buffer.id);
	const indexById = new Map<string, number>(
		orderHint.map((id, index) => [id, index] as const)
	);
	const indegree = new Map<string, number>(orderHint.map((id) => [id, 0] as const));
	const adjacency = new Map<string, Set<string>>(
		orderHint.map((id) => [id, new Set<string>()] as const)
	);

	for (const pass of graph.buffers) {
		for (const channel of pass.channels) {
			if (!channel || channel.type !== "pass") {
				continue;
			}
			if (channel.passId === pass.id) {
				continue;
			}
			if (!indegree.has(channel.passId)) {
				continue;
			}

			const dependents = adjacency.get(channel.passId);
			if (!dependents || dependents.has(pass.id)) {
				continue;
			}

			dependents.add(pass.id);
			indegree.set(pass.id, (indegree.get(pass.id) ?? 0) + 1);
		}
	}

	const queue = orderHint
		.filter((id) => (indegree.get(id) ?? 0) === 0)
		.sort((a, b) => (indexById.get(a) ?? 0) - (indexById.get(b) ?? 0));
	const order: string[] = [];

	while (queue.length > 0) {
		const id = queue.shift();
		if (!id) {
			break;
		}
		order.push(id);

		const dependents = adjacency.get(id);
		if (!dependents) {
			continue;
		}
		for (const dependent of dependents) {
			const nextIndegree = (indegree.get(dependent) ?? 0) - 1;
			indegree.set(dependent, nextIndegree);
			if (nextIndegree === 0) {
				queue.push(dependent);
				queue.sort((a, b) => (indexById.get(a) ?? 0) - (indexById.get(b) ?? 0));
			}
		}
	}

	if (order.length !== graph.buffers.length) {
		throw new Error("Shader buffers contain cyclic pass dependencies.");
	}

	return order;
};

/**
 * Compiles a shader from source code.
 */
const createShader = (
	gl: WebGL2RenderingContext,
	type: number,
	source: string
): WebGLShader | null => {
	const shader = gl.createShader(type);
	if (!shader) {
		logn.error("shader", "Unable to create shader: gl.createShader returned null");
		return null;
	}
	gl.shaderSource(shader, source);
	gl.compileShader(shader);
	const success = gl.getShaderParameter(shader, gl.COMPILE_STATUS);
	if (success) {
		return shader;
	}
	logn.error("shader", "Failed to compile shader", gl.getShaderInfoLog(shader));
	gl.deleteShader(shader);
	return null;
};

/**
 * Links a vertex and fragment shader into a WebGL program.
 */
const createProgram = (
	gl: WebGL2RenderingContext,
	vertexShader: WebGLShader,
	fragmentShader: WebGLShader
): WebGLProgram | null => {
	const program = gl.createProgram();
	if (!program) {
		logn.error("shader", "Unable to create program: gl.createProgram returned null");
		return null;
	}
	gl.attachShader(program, vertexShader);
	gl.attachShader(program, fragmentShader);
	gl.linkProgram(program);
	const success = gl.getProgramParameter(program, gl.LINK_STATUS);
	if (success) {
		return program;
	}
	logn.error("shader", "Failed to link program", gl.getProgramInfoLog(program));
	gl.deleteProgram(program);
	return null;
};

/**
 * Checks if the canvas needs to be resized and applies the new dimensions.
 */
const resizeCanvasToDisplaySize = (canvas: HTMLCanvasElement): boolean => {
	const displayWidth = canvas.clientWidth;
	const displayHeight = canvas.clientHeight;

	if (canvas.width !== displayWidth || canvas.height !== displayHeight) {
		canvas.width = displayWidth;
		canvas.height = displayHeight;
		return true;
	}
	return false;
};

const getCanvasSize = (canvas: HTMLCanvasElement) => {
	return {
		width: Math.max(1, canvas.width || canvas.clientWidth || 1),
		height: Math.max(1, canvas.height || canvas.clientHeight || 1)
	};
};

const createFallbackTexture = (gl: WebGL2RenderingContext): WebGLTexture | null => {
	const texture = gl.createTexture();
	if (!texture) {
		return null;
	}
	gl.bindTexture(gl.TEXTURE_2D, texture);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
	gl.texImage2D(
		gl.TEXTURE_2D,
		0,
		gl.RGBA,
		1,
		1,
		0,
		gl.RGBA,
		gl.UNSIGNED_BYTE,
		new Uint8Array([0, 0, 0, 255])
	);
	gl.bindTexture(gl.TEXTURE_2D, null);
	return texture;
};

const loadImage = async (url: string): Promise<HTMLImageElement> => {
	return await new Promise<HTMLImageElement>((resolve, reject) => {
		const image = new Image();
		image.crossOrigin = "anonymous";
		image.onload = () => resolve(image);
		image.onerror = () => reject(new Error(`Failed to load image: ${url}`));
		image.src = url;
	});
};

const loadTextureFromUrl = async (
	gl: WebGL2RenderingContext,
	url: string
): Promise<WebGLTexture | null> => {
	try {
		const image = await loadImage(url);
		const texture = gl.createTexture();
		if (!texture) {
			return null;
		}

		gl.bindTexture(gl.TEXTURE_2D, texture);
		gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
		gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
		gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
		gl.bindTexture(gl.TEXTURE_2D, null);

		return texture;
	} catch (error) {
		logn.error("shader", "Failed to load texture input", {
			url,
			error
		});
		return null;
	}
};

const resolveShaderSource = async (source: ShaderPassSource): Promise<string> => {
	if ("inline" in source && typeof source.inline === "string" && source.inline) {
		return source.inline;
	}
	if ("url" in source && typeof source.url === "string" && source.url) {
		return fetchShader(source.url);
	}
	return "";
};

const getTextureUrls = (graph: NormalizedShaderGraph): string[] => {
	const urls = new Set<string>();
	for (const pass of [graph.image, ...graph.buffers]) {
		for (const channel of pass.channels) {
			if (channel?.type === "texture") {
				urls.add(channel.url);
			}
		}
	}
	return [...urls];
};

const createRenderTarget = (
	gl: WebGL2RenderingContext,
	width: number,
	height: number
): RenderTarget | null => {
	const texture = gl.createTexture();
	const framebuffer = gl.createFramebuffer();
	if (!texture || !framebuffer) {
		if (texture) {
			gl.deleteTexture(texture);
		}
		if (framebuffer) {
			gl.deleteFramebuffer(framebuffer);
		}
		return null;
	}

	gl.bindTexture(gl.TEXTURE_2D, texture);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
	gl.texImage2D(
		gl.TEXTURE_2D,
		0,
		gl.RGBA,
		width,
		height,
		0,
		gl.RGBA,
		gl.UNSIGNED_BYTE,
		null
	);

	gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
	gl.framebufferTexture2D(
		gl.FRAMEBUFFER,
		gl.COLOR_ATTACHMENT0,
		gl.TEXTURE_2D,
		texture,
		0
	);

	const isComplete =
		gl.checkFramebufferStatus(gl.FRAMEBUFFER) === gl.FRAMEBUFFER_COMPLETE;
	gl.bindTexture(gl.TEXTURE_2D, null);
	gl.bindFramebuffer(gl.FRAMEBUFFER, null);

	if (!isComplete) {
		gl.deleteTexture(texture);
		gl.deleteFramebuffer(framebuffer);
		return null;
	}

	return { framebuffer, texture };
};

const resizeRenderTarget = (
	gl: WebGL2RenderingContext,
	target: RenderTarget,
	width: number,
	height: number
) => {
	gl.bindTexture(gl.TEXTURE_2D, target.texture);
	gl.texImage2D(
		gl.TEXTURE_2D,
		0,
		gl.RGBA,
		width,
		height,
		0,
		gl.RGBA,
		gl.UNSIGNED_BYTE,
		null
	);
	gl.bindTexture(gl.TEXTURE_2D, null);
};

const createFullscreenGeometry = (
	gl: WebGL2RenderingContext,
	s: ShaderState
): boolean => {
	const positionBuffer = gl.createBuffer();
	const vao = gl.createVertexArray();
	if (!positionBuffer || !vao) {
		logn.error("shader", "Failed to create fullscreen geometry resources.");
		if (positionBuffer) {
			gl.deleteBuffer(positionBuffer);
		}
		if (vao) {
			gl.deleteVertexArray(vao);
		}
		return false;
	}

	s.positionBuffer = positionBuffer;
	s.vao = vao;

	gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
	gl.bufferData(
		gl.ARRAY_BUFFER,
		new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
		gl.STATIC_DRAW
	);

	gl.bindVertexArray(vao);
	gl.enableVertexAttribArray(POSITION_ATTRIBUTE_LOCATION);
	gl.vertexAttribPointer(POSITION_ATTRIBUTE_LOCATION, 2, gl.FLOAT, false, 0, 0);
	gl.bindVertexArray(null);
	gl.bindBuffer(gl.ARRAY_BUFFER, null);

	return true;
};

const createPassProgram = (
	gl: WebGL2RenderingContext,
	mainImageShader: string
): {
	program: WebGLProgram;
	vertexShader: WebGLShader;
	fragmentShader: WebGLShader;
	uniformLocations: UniformLocations;
} | null => {
	const vertexShader = createShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SOURCE);
	const fragmentShader = createShader(
		gl,
		gl.FRAGMENT_SHADER,
		createFragmentShaderSource(mainImageShader)
	);
	if (!vertexShader || !fragmentShader) {
		if (vertexShader) {
			gl.deleteShader(vertexShader);
		}
		if (fragmentShader) {
			gl.deleteShader(fragmentShader);
		}
		return null;
	}

	const program = createProgram(gl, vertexShader, fragmentShader);
	if (!program) {
		gl.deleteShader(vertexShader);
		gl.deleteShader(fragmentShader);
		return null;
	}

	const uniformLocations: UniformLocations = {
		iResolution: gl.getUniformLocation(program, "iResolution"),
		iTime: gl.getUniformLocation(program, "iTime"),
		iTimeDelta: gl.getUniformLocation(program, "iTimeDelta"),
		iFrame: gl.getUniformLocation(program, "iFrame"),
		iMouse: gl.getUniformLocation(program, "iMouse"),
		iDate: gl.getUniformLocation(program, "iDate"),
		iFrameRate: gl.getUniformLocation(program, "iFrameRate"),
		iChannels: Array.from({ length: CHANNEL_COUNT }, (_, index) =>
			gl.getUniformLocation(program, `iChannel${index}`)
		)
	};

	return { program, vertexShader, fragmentShader, uniformLocations };
};

const resolveChannelTexture = (
	s: ShaderState,
	pass: CompiledPass,
	channel: ShaderChannelInput | null,
	renderedBufferPasses: Set<string>
): WebGLTexture | null => {
	if (!channel) {
		return s.fallbackTexture;
	}

	if (channel.type === "texture") {
		return s.loadedTextures.get(channel.url) ?? s.fallbackTexture;
	}

	const referencedPass = s.bufferPassesById.get(channel.passId);
	if (!referencedPass) {
		return s.fallbackTexture;
	}

	if (channel.passId === pass.id) {
		return referencedPass.readTarget.texture;
	}

	if (renderedBufferPasses.has(channel.passId)) {
		return referencedPass.writeTarget.texture;
	}

	return referencedPass.readTarget.texture;
};

const renderPass = (
	s: ShaderState,
	pass: CompiledPass,
	framebuffer: WebGLFramebuffer | null,
	renderedBufferPasses: Set<string>,
	elapsedTime: number,
	deltaTime: number,
	frameRate: number,
	frame: number,
	width: number,
	height: number
) => {
	const gl = s.gl;
	if (!gl || !s.vao) {
		return;
	}

	gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
	gl.viewport(0, 0, width, height);
	gl.clearColor(0, 0, 0, 0);
	gl.clear(gl.COLOR_BUFFER_BIT);
	gl.useProgram(pass.program);
	gl.bindVertexArray(s.vao);

	gl.uniform3f(pass.uniformLocations.iResolution, width, height, 1.0);
	gl.uniform1f(pass.uniformLocations.iTime, elapsedTime);
	gl.uniform1f(pass.uniformLocations.iTimeDelta, deltaTime);
	gl.uniform1i(pass.uniformLocations.iFrame, frame);
	gl.uniform4f(pass.uniformLocations.iMouse, 0, 0, 0, 0);
	gl.uniform1f(pass.uniformLocations.iFrameRate, frameRate);

	const date = new Date();
	const seconds =
		date.getHours() * 3600 +
		date.getMinutes() * 60 +
		date.getSeconds() +
		date.getMilliseconds() / 1000;
	gl.uniform4f(
		pass.uniformLocations.iDate,
		date.getFullYear(),
		date.getMonth(),
		date.getDate(),
		seconds
	);

	for (let channelIndex = 0; channelIndex < CHANNEL_COUNT; channelIndex++) {
		const uniformLocation = pass.uniformLocations.iChannels[channelIndex];
		const channel = pass.channels[channelIndex];
		const texture = resolveChannelTexture(s, pass, channel, renderedBufferPasses);
		gl.activeTexture(gl.TEXTURE0 + channelIndex);
		gl.bindTexture(gl.TEXTURE_2D, texture);
		gl.uniform1i(uniformLocation, channelIndex);
	}

	gl.drawArrays(gl.TRIANGLES, 0, 6);
};

const resizeBufferTargets = (s: ShaderState, width: number, height: number) => {
	if (!s.gl) {
		return;
	}
	for (const bufferPass of s.bufferPassesById.values()) {
		resizeRenderTarget(s.gl, bufferPass.readTarget, width, height);
		resizeRenderTarget(s.gl, bufferPass.writeTarget, width, height);
	}
};

const render = (s: ShaderState, now: DOMHighResTimeStamp) => {
	if (s.isDisposed || !s.gl || !s.canvas || !s.imagePass) {
		return;
	}

	const resized = resizeCanvasToDisplaySize(s.canvas);
	const { width, height } = getCanvasSize(s.canvas);
	if (resized) {
		resizeBufferTargets(s, width, height);
	}

	const elapsedTime = (now - s.startTime) / 1000;
	const deltaTime = (now - s.frameTime) / 1000;
	const frameRate = deltaTime > 0 ? 1 / deltaTime : 0;
	const frame = s.frameCount;
	s.frameTime = now;

	const renderedBufferPasses = new Set<string>();

	for (const passId of s.bufferPassOrder) {
		const bufferPass = s.bufferPassesById.get(passId);
		if (!bufferPass) {
			continue;
		}
		renderPass(
			s,
			bufferPass,
			bufferPass.writeTarget.framebuffer,
			renderedBufferPasses,
			elapsedTime,
			deltaTime,
			frameRate,
			frame,
			width,
			height
		);
		renderedBufferPasses.add(passId);
	}

	renderPass(
		s,
		s.imagePass,
		null,
		renderedBufferPasses,
		elapsedTime,
		deltaTime,
		frameRate,
		frame,
		width,
		height
	);

	for (const bufferPass of s.bufferPassesById.values()) {
		const nextRead = bufferPass.writeTarget;
		bufferPass.writeTarget = bufferPass.readTarget;
		bufferPass.readTarget = nextRead;
	}

	s.frameCount += 1;
	s.rafId = requestAnimationFrame((frameNow: DOMHighResTimeStamp) =>
		render(s, frameNow)
	);
};

const clearStateCollections = (s: ShaderState) => {
	s.loadedTextures.clear();
	s.bufferPassesById.clear();
	s.bufferPassOrder = [];
	s.imagePass = null;
	s.programs = [];
	s.framebuffers = [];
	s.renderTextures = [];
};

export const teardown = (s: ShaderState) => {
	s.isDisposed = true;

	if (s.rafId !== null) {
		cancelAnimationFrame(s.rafId);
		s.rafId = null;
	}

	if (!s.gl) {
		clearStateCollections(s);
		s.canvas = null;
		return;
	}

	for (const texture of s.loadedTextures.values()) {
		s.gl.deleteTexture(texture);
	}
	for (const texture of s.renderTextures) {
		s.gl.deleteTexture(texture);
	}
	if (s.fallbackTexture) {
		s.gl.deleteTexture(s.fallbackTexture);
	}
	for (const framebuffer of s.framebuffers) {
		s.gl.deleteFramebuffer(framebuffer);
	}
	for (const program of s.programs) {
		s.gl.deleteProgram(program);
	}
	if (s.positionBuffer) {
		s.gl.deleteBuffer(s.positionBuffer);
	}
	if (s.vao) {
		s.gl.deleteVertexArray(s.vao);
	}

	s.gl.bindVertexArray(null);
	s.gl.bindBuffer(s.gl.ARRAY_BUFFER, null);
	s.gl.bindFramebuffer(s.gl.FRAMEBUFFER, null);
	s.gl.useProgram(null);

	clearStateCollections(s);
	s.fallbackTexture = null;
	s.positionBuffer = null;
	s.vao = null;
	s.gl = null;
	s.canvas = null;
};

const collectPasses = (graph: NormalizedShaderGraph): NormalizedShaderPass[] => {
	return [...graph.buffers, graph.image];
};

const getPassById = (
	graph: NormalizedShaderGraph,
	id: string
): NormalizedShaderPass | undefined => {
	if (id === IMAGE_PASS_ID) {
		return graph.image;
	}
	return graph.buffers.find((pass) => pass.id === id);
};

/**
 * The main setup function.
 *
 * Resolves graph assets, compiles pass programs and starts the render loop.
 */
export const setup = async (
	graph: NormalizedShaderGraph,
	s: ShaderState,
	canvas: HTMLCanvasElement
) => {
	s.isDisposed = false;
	s.canvas = canvas;
	s.gl = canvas.getContext("webgl2");
	if (!s.gl) {
		logn.error("shader", "WebGL 2 not supported.");
		return;
	}
	const gl = s.gl;

	try {
		resizeCanvasToDisplaySize(canvas);
		const { width, height } = getCanvasSize(canvas);

		if (!createFullscreenGeometry(gl, s)) {
			return;
		}

		s.fallbackTexture = createFallbackTexture(gl);

		const shaderCodeByPassId = new Map<string, string>();
		for (const pass of collectPasses(graph)) {
			const shaderCode = await resolveShaderSource(pass.shader);
			if (s.isDisposed) {
				return;
			}
			if (!shaderCode) {
				throw new Error(`Shader source for pass "${pass.id}" is empty.`);
			}
			shaderCodeByPassId.set(pass.id, shaderCode);
		}

		const textureUrls = getTextureUrls(graph);
		const textureEntries = await Promise.all(
			textureUrls.map(async (url) => {
				const texture = await loadTextureFromUrl(gl, url);
				return [url, texture] as const;
			})
		);
		if (s.isDisposed) {
			for (const [, texture] of textureEntries) {
				if (texture) {
					gl.deleteTexture(texture);
				}
			}
			return;
		}
		for (const [url, texture] of textureEntries) {
			if (!texture) {
				continue;
			}
			s.loadedTextures.set(url, texture);
		}

		s.bufferPassOrder = resolveBufferPassOrder(graph);

		for (const passId of [...s.bufferPassOrder, IMAGE_PASS_ID]) {
			const pass = getPassById(graph, passId);
			const shaderCode = shaderCodeByPassId.get(passId);
			if (!pass || !shaderCode) {
				throw new Error(`Unable to resolve shader pass "${passId}".`);
			}

			const compiled = createPassProgram(gl, shaderCode);
			if (!compiled) {
				throw new Error(`Failed to compile pass "${passId}".`);
			}

			s.programs.push(compiled.program);
			gl.deleteShader(compiled.vertexShader);
			gl.deleteShader(compiled.fragmentShader);

			if (passId === IMAGE_PASS_ID) {
				s.imagePass = {
					id: pass.id,
					program: compiled.program,
					uniformLocations: compiled.uniformLocations,
					channels: [...pass.channels]
				};
				continue;
			}

			const readTarget = createRenderTarget(gl, width, height);
			const writeTarget = createRenderTarget(gl, width, height);
			if (!readTarget || !writeTarget) {
				throw new Error(`Failed to create render targets for pass "${pass.id}".`);
			}

			s.framebuffers.push(readTarget.framebuffer, writeTarget.framebuffer);
			s.renderTextures.push(readTarget.texture, writeTarget.texture);
			s.bufferPassesById.set(pass.id, {
				id: pass.id,
				program: compiled.program,
				uniformLocations: compiled.uniformLocations,
				channels: [...pass.channels],
				readTarget,
				writeTarget
			});
		}

		if (!s.imagePass) {
			throw new Error("Image pass did not initialize.");
		}

		s.startTime = performance.now();
		s.frameTime = s.startTime;
		s.frameCount = 0;
		s.rafId = requestAnimationFrame((frameNow: DOMHighResTimeStamp) =>
			render(s, frameNow)
		);
	} catch (error) {
		logn.error("shader", "Failed to setup shader graph.", error);
		teardown(s);
	}
};
