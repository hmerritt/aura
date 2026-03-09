import * as stylex from "@stylexjs/stylex";
import { RefObject, useCallback, useEffect, useRef } from "react";

import { type SxProp } from "lib/type-assertions";

export type DotGridProps = React.JSX.IntrinsicElements["canvas"] &
	SxProp & {
		/** Container position. Use `fixed` for background usage */
		position?: "absolute" | "fixed";
		/** Spacing between the dots */
		spacing?: number;
		/** Size of each dot */
		dotSize?: number;
		/** Damping for smoother motion */
		damping?: number;
		/** Speed at which dots return to their original position */
		returnSpeed?: number;
		/** Base for the exponential function */
		attractionBase?: number;
		/** Maximum attraction to avoid extreme values */
		maxAttraction?: number;
		/** ref of element to use for mouse position (leave undefined to use the canvas) */
		refForMousePosition?: RefObject<any> | "window";
		/** Redraw canvas on window resize (responsive, but may impact performance) */
		reactToWindowResize?: boolean;
	};

/**
 * Grid of dots that are attracted to the mouse position.
 *
 * Inspired by https://twitter.com/eliguerron/status/1738116017631740213
 */
export const DotGrid: React.FC<DotGridProps> = ({
	sx,
	position = "absolute",
	spacing: spacingProps = 40,
	dotSize = 1,
	damping = 0.45,
	returnSpeed = 0.18,
	attractionBase = 1.03,
	maxAttraction = 0.6,
	refForMousePosition,
	reactToWindowResize = true,
	...canvasProps
}) => {
	const $canvas = useRef<HTMLCanvasElement>(null);
	const animationFrameHandle = useRef(-1);
	const mousePosition = useRef({ x: -1000, y: -1000 });

	const resetAnimationFrame = () => {
		if (animationFrameHandle.current !== -1) {
			cancelAnimationFrame(animationFrameHandle.current); // Cancel previous frame
			animationFrameHandle.current = -1;
		}
	};

	const drawDotGrid = useCallback(() => {
		if (!$canvas.current) return;
		resetAnimationFrame();

		const canvas = $canvas.current;
		if (!canvas?.getContext) return; // Tests fail without this

		const ctx = canvas?.getContext("2d");
		const dpr = window.devicePixelRatio || 1;

		ctx?.scale(dpr, dpr);

		// Set canvas size
		if (refForMousePosition === "window") {
			canvas.width = window.innerWidth;
			canvas.height = window.innerHeight;
		} else {
			// Get parent element size
			const parentElement = canvas.parentElement;
			canvas.width = parentElement?.offsetWidth || window.innerWidth;
			canvas.height = parentElement?.offsetHeight || window.innerHeight;
		}

		const dots: {
			x: number;
			y: number;
			vx: number;
			vy: number;
			originalX: number;
			originalY: number;
		}[] = [];

		// Initialize dots array
		const spacing = Math.max(5, spacingProps); // Minimum spacing
		const xySpacing = Math.round(spacing + spacing / 2);
		for (let x = -xySpacing; x < canvas.width + xySpacing; x += spacing) {
			for (let y = -xySpacing; y < canvas.height + xySpacing; y += spacing) {
				dots.push({
					x: x,
					y: y,
					vx: 0,
					vy: 0,
					originalX: x,
					originalY: y
				});
			}
		}

		function draw() {
			if (!ctx) return;
			ctx.clearRect(0, 0, canvas.width, canvas.height);
			for (const dot of dots) {
				const dx = mousePosition.current.x - dot.x;
				const dy = mousePosition.current.y - dot.y;
				const distance = Math.sqrt(dx * dx + dy * dy);

				// Exponential attraction calculation
				const attractionFactor = Math.min(
					Math.pow(attractionBase, -distance),
					maxAttraction
				);

				if (distance > 1) {
					// Avoid extreme values near mouse
					dot.vx += dx * attractionFactor;
					dot.vy += dy * attractionFactor;
				}

				// Apply return force and damping
				dot.vx += (dot.originalX - dot.x) * returnSpeed;
				dot.vy += (dot.originalY - dot.y) * returnSpeed;
				dot.vx *= damping;
				dot.vy *= damping;

				// Update position
				dot.x += dot.vx;
				dot.y += dot.vy;

				drawDot(dot.x, dot.y, dotSize);
			}
			animationFrameHandle.current = requestAnimationFrame(draw);
		}

		function drawDot(x: number, y: number, size: number) {
			if (!ctx) return;
			ctx.beginPath();
			ctx.arc(Math.round(x), Math.round(y), size, 0, Math.PI * 2, false);
			ctx.imageSmoothingEnabled = false;
			ctx.fillStyle = "black";
			ctx.fill();
		}

		draw();
	}, [
		refForMousePosition,
		spacingProps,
		attractionBase,
		maxAttraction,
		returnSpeed,
		damping,
		dotSize
	]);

	useEffect(() => {
		if (!$canvas.current) return;

		drawDotGrid();

		if (reactToWindowResize) {
			window.addEventListener("resize", drawDotGrid);
		}

		const trackMousePosition = (e: MouseEvent | TouchEvent) => {
			if (!$canvas.current) return;

			let x = mousePosition.current.x;
			let y = mousePosition.current.y;

			if (e.type === "mousemove") {
				const native = e as MouseEvent;
				x = native?.offsetX;
				y = native?.offsetY;
				if (refForMousePosition === "window") {
					x = native?.clientX;
					y = native?.clientY;
				}
			} else if (e.type === "touchmove") {
				const native = e as TouchEvent;
				const bcr = $canvas.current.getBoundingClientRect();
				const touch = native?.touches?.[0] ?? native?.targetTouches?.[0];
				x = touch.clientX - bcr.x;
				y = touch.clientY - bcr.y;
				if (refForMousePosition === "window") {
					x = touch.clientX;
					y = touch.clientY;
				}
			} else if (e.type === "mouseout" || e.type === "touchend") {
				setTimeout(() => {
					// If mouse position has changed, abort reset (this means mouse has moved back into tracking area)
					if (x !== mousePosition.current.x || y !== mousePosition.current.y)
						return;

					x = -1000;
					y = -1000;
					mousePosition.current.x = x;
					mousePosition.current.y = y;
				}, 600);
			}

			mousePosition.current.x = x;
			mousePosition.current.y = y;
		};
		const $elForMousePosition =
			refForMousePosition === "window"
				? window
				: refForMousePosition?.current || $canvas.current;
		$elForMousePosition.addEventListener("mousemove", trackMousePosition);
		$elForMousePosition.addEventListener("mouseout", trackMousePosition);
		$elForMousePosition.addEventListener("touchmove", trackMousePosition);
		$elForMousePosition.addEventListener("touchend", trackMousePosition);

		return () => {
			resetAnimationFrame();
			window.removeEventListener("resize", drawDotGrid);
			$elForMousePosition?.removeEventListener("mousemove", trackMousePosition);
			$elForMousePosition?.removeEventListener("mouseout", trackMousePosition);
			$elForMousePosition?.removeEventListener("touchmove", trackMousePosition);
			$elForMousePosition?.removeEventListener("touchend", trackMousePosition);
		};
	}, [drawDotGrid, reactToWindowResize, refForMousePosition]);

	return (
		<canvas
			{...canvasProps}
			ref={$canvas}
			{...stylex.props(
				styles.dotGrid,
				position === "fixed" && styles.dotGridFixed,
				sx
			)}
		/>
	);
};

const styles = stylex.create({
	dotGrid: {
		display: "block",
		inset: 0,
		position: "absolute"
	},
	dotGridFixed: {
		position: "fixed"
	}
});
