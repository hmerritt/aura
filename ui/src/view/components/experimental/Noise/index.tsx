import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef } from "react";

import { type SxProp } from "lib/type-assertions";

import { Image, ImageProps } from "view/components";

const canvasNoise = (
	ctx: CanvasRenderingContext2D,
	patternSize = 64,
	patternAlpha = 25
) => {
	const patternPixelDataLength = patternSize * patternSize * 4;
	const patternData = ctx.createImageData(patternSize, patternSize);

	for (let i = 0; i < patternPixelDataLength; i += 4) {
		const value: number = (Math.random() * 255) | 0;
		patternData.data[i] = value;
		patternData.data[i + 1] = value;
		patternData.data[i + 2] = value;
		patternData.data[i + 3] = patternAlpha;
	}

	ctx.putImageData(patternData, 0, 0);
};

const canvasResize = (canvas: HTMLCanvasElement, patternSize = 64) => {
	canvas.style.width = "100%";
	canvas.style.height = "100%";
	canvas.width = patternSize;
	canvas.height = patternSize;
};

export type NoiseProps = SxProp & {
	framerate?: number;
	size?: number;
	alpha?: number;
	reactToWindowResize?: boolean;
};

export type NoiseImgProps = NoiseProps &
	React.JSX.IntrinsicElements["div"] & {
		src?: string;
		imgProps?: ImageProps;
		childrenIsAboveNoise?: boolean;
	};

/**
 * Animated noise effect.
 *
 * @warning can negatively impact performance
 */
export const Noise = ({
	sx,
	framerate = 12,
	size = 256,
	alpha = 25,
	reactToWindowResize = true
}: NoiseProps) => {
	const $canvas = useRef<HTMLCanvasElement>(null);
	const loopIsRunning = useRef(true);

	useEffect(() => {
		const canvas = $canvas.current;
		const ctx = canvas?.getContext?.("2d");
		if (!ctx || !canvas?.getContext) return;

		const runCanResize = () => canvasResize(canvas, size);

		runCanResize();

		if (reactToWindowResize) {
			window.addEventListener("resize", runCanResize);
		}

		loopIsRunning.current = true;
		const fpsInterval = 1000 / framerate;
		let then = Date.now();

		function loop() {
			if (!canvas || !ctx || !loopIsRunning.current) return;
			requestAnimationFrame(loop);

			const now = Date.now();
			const elapsed = now - then;
			if (elapsed > fpsInterval) {
				then = now - (elapsed % fpsInterval);
				canvasNoise(ctx, size, alpha);
			}
		}
		loop();

		return () => {
			loopIsRunning.current = false;
			window.removeEventListener("resize", runCanResize);
		};
	}, [alpha, framerate, reactToWindowResize, size]);

	return <canvas ref={$canvas} {...stylex.props(styles.canvasStyle, sx)} />;
};

/**
 * Img wrapped with Noise.
 */
export const NoiseImg = ({
	// Noise
	framerate = 12,
	size = 256,
	alpha = 25,
	reactToWindowResize = true,
	// Img
	src,
	imgProps,
	// NoiseImg
	sx,
	children,
	childrenIsAboveNoise = true,
	...divProps
}: NoiseImgProps) => {
	return (
		<div {...divProps} {...stylex.props(styles.noiseImg, sx)}>
			<Image
				src={src}
				width="100%"
				height="100%"
				hideWhileLoading={true}
				{...imgProps}
			/>
			<Noise
				framerate={framerate}
				size={size}
				alpha={alpha}
				reactToWindowResize={reactToWindowResize}
			/>
			{children && (
				<div
					{...stylex.props(
						styles.noiseImgChildren,
						childrenIsAboveNoise
							? styles.noiseImgChildrenAbove
							: styles.noiseImgChildrenBelow
					)}
				>
					{children}
				</div>
			)}
		</div>
	);
};

const styles = stylex.create({
	canvasStyle: {
		height: "100%",
		left: 0,
		pointerEvents: "none",
		position: "absolute",
		top: 0,
		userSelect: "none",
		width: "100%",
		zIndex: 10
	},
	noiseImg: {
		position: "relative"
	},
	noiseImgChildren: {
		height: "100%",
		left: 0,
		position: "absolute",
		top: 0,
		width: "100%"
	},
	noiseImgChildrenAbove: {
		zIndex: 20
	},
	noiseImgChildrenBelow: {
		zIndex: 5
	}
});
