import * as stylex from "@stylexjs/stylex";
import {
	type MouseEvent,
	type PointerEvent,
	type TouchEvent,
	useEffect,
	useRef
} from "react";

import { type SxProp } from "lib/type-assertions";

type RippleRecord = {
	id: number;
	pointerId: number;
	container: HTMLSpanElement;
	ripple: HTMLSpanElement;
	isRemoving: boolean;
	removeTimeout: number | null;
};

export type RippleProps = React.JSX.IntrinsicElements["div"] &
	SxProp & {
		hoverBg?: boolean;
		centered?: boolean;
		disabled?: boolean;
		color?: string;
		hoverColor?: string;
	};

/**
 * Animated ripple effect on-press (inspired by material-ui).
 */
export const Ripple = ({
	sx,
	children,
	hoverBg,
	centered,
	disabled,
	color = "rgba(0, 0, 0, 0.1)",
	hoverColor = "#f5f5f5",
	onMouseDown,
	onMouseUp,
	onTouchStart,
	onTouchEnd,
	onPointerDown,
	onPointerUp,
	onPointerCancel,
	onLostPointerCapture,
	...props
}: RippleProps) => {
	const count = useRef(0);
	const ripples = useRef<Map<number, RippleRecord>>(new Map());
	const pointerToRipple = useRef<Map<number, number>>(new Map());

	const removeRippleRecord = (record: RippleRecord) => {
		if (record.isRemoving) {
			return;
		}

		record.isRemoving = true;
		Object.assign(record.ripple.style, {
			transitionDuration: "220ms",
			opacity: "0"
		});

		record.removeTimeout = window.setTimeout(() => {
			record.container.remove();
			ripples.current.delete(record.id);

			const mappedRippleId = pointerToRipple.current.get(record.pointerId);
			if (mappedRippleId === record.id) {
				pointerToRipple.current.delete(record.pointerId);
			}
		}, 240);
	};

	const removeRippleByPointer = (pointerId: number) => {
		const rippleId = pointerToRipple.current.get(pointerId);
		if (typeof rippleId !== "number") {
			return;
		}

		pointerToRipple.current.delete(pointerId);

		const record = ripples.current.get(rippleId);
		if (!record) {
			return;
		}

		removeRippleRecord(record);
	};

	useEffect(() => {
		const ripplesMap = ripples.current;
		const pointerMap = pointerToRipple.current;

		return () => {
			for (const ripple of ripplesMap.values()) {
				if (ripple.removeTimeout !== null) {
					clearTimeout(ripple.removeTimeout);
				}
				ripple.container.remove();
			}
			ripplesMap.clear();
			pointerMap.clear();
		};
	}, []);

	const createRipple = (e: PointerEvent<HTMLDivElement>) => {
		const rippleId = count.current++;
		const button = e.currentTarget;
		const style = window.getComputedStyle(button);
		const dimensions = button.getBoundingClientRect();

		const pointerX = Math.round(e.clientX - dimensions.left) || 0;
		const pointerY = Math.round(e.clientY - dimensions.top) || 0;

		const touchX = centered || !pointerX ? dimensions.width / 2 : pointerX;
		const touchY = centered || !pointerY ? dimensions.height / 2 : pointerY;

		const size = Math.max(dimensions.width, dimensions.height) * 2.5;
		const container = document.createElement("span");
		container.setAttribute("data-ripple", "");
		container.setAttribute(`data-ripple-${rippleId}`, "");

		Object.assign(container.style, {
			position: "absolute",
			pointerEvents: "none",
			top: "0",
			left: "0",
			right: "0",
			bottom: "0",
			borderTopLeftRadius: style.borderTopLeftRadius,
			borderTopRightRadius: style.borderTopRightRadius,
			borderBottomRightRadius: style.borderBottomRightRadius,
			borderBottomLeftRadius: style.borderBottomLeftRadius
		});

		const ripple = document.createElement("span");
		const mouseX = e.clientX - dimensions.left;
		const centerX = button.clientWidth / 2;
		const distanceToCenter =
			centerX > 0 ? Math.abs(((mouseX - centerX) / centerX) * 100) : 0;
		const durationScaler = centered ? 1 : distanceToCenter * 0.4;
		const duration = Math.max(120, 280 - (280 * durationScaler) / 100);

		Object.assign(ripple.style, {
			position: "absolute",
			pointerEvents: "none",
			backgroundColor: color,
			borderRadius: "50%",
			zIndex: "10",
			transitionProperty: "transform opacity",
			transitionDuration: `${duration}ms`,
			transitionTimingFunction: "linear",
			transformOrigin: "center",
			transform: "translate3d(-50%, -50%, 0) scale3d(0.15, 0.15, 0.1)",
			opacity: "0.5",
			left: `${touchX}px`,
			top: `${touchY}px`,
			width: `${size}px`,
			height: `${size}px`
		});

		container.appendChild(ripple);
		button.appendChild(container);

		requestAnimationFrame(() => {
			requestAnimationFrame(() => {
				Object.assign(ripple.style, {
					transform: "translate3d(-50%, -50%, 0) scale3d(1, 1, 1)",
					opacity: "1"
				});
			});
		});

		const record: RippleRecord = {
			id: rippleId,
			pointerId: e.pointerId,
			container,
			ripple,
			isRemoving: false,
			removeTimeout: null
		};

		ripples.current.set(rippleId, record);
		pointerToRipple.current.set(e.pointerId, rippleId);
	};

	const handlePointerDown = (e: PointerEvent<HTMLDivElement>) => {
		if (disabled) {
			return;
		}

		onPointerDown?.(e);

		if (e.pointerType === "touch") {
			onTouchStart?.(e as unknown as TouchEvent<HTMLDivElement>);
		} else {
			onMouseDown?.(e as unknown as MouseEvent<HTMLDivElement>);
		}

		if (e.currentTarget.setPointerCapture) {
			try {
				e.currentTarget.setPointerCapture(e.pointerId);
			} catch {
				// Not all targets can capture pointers in every environment.
			}
		}

		createRipple(e);
	};

	const handlePointerUp = (e: PointerEvent<HTMLDivElement>) => {
		if (disabled) {
			return;
		}

		onPointerUp?.(e);

		if (e.pointerType === "touch") {
			onTouchEnd?.(e as unknown as TouchEvent<HTMLDivElement>);
		} else {
			onMouseUp?.(e as unknown as MouseEvent<HTMLDivElement>);
		}

		removeRippleByPointer(e.pointerId);
	};

	const handlePointerCancel = (e: PointerEvent<HTMLDivElement>) => {
		if (disabled) {
			return;
		}

		onPointerCancel?.(e);
		removeRippleByPointer(e.pointerId);
	};

	const handleLostPointerCapture = (e: PointerEvent<HTMLDivElement>) => {
		if (disabled) {
			return;
		}

		onLostPointerCapture?.(e);
		removeRippleByPointer(e.pointerId);
	};

	return (
		<div
			{...props}
			{...stylex.props(
				styles.ripple,
				hoverBg && !disabled && styles.rippleHover(hoverColor),
				disabled && styles.rippleDisabled,
				sx
			)}
			onPointerDown={handlePointerDown}
			onPointerUp={handlePointerUp}
			onPointerCancel={handlePointerCancel}
			onLostPointerCapture={handleLostPointerCapture}
		>
			{children}
		</div>
	);
};

const styles = stylex.create({
	ripple: {
		cursor: "pointer",
		overflow: "hidden",
		position: "relative",
		transition: "200ms background-color",
		willChange: "background-color"
	},
	rippleHover: (backgroundColor: string) => ({ backgroundColor }),
	rippleDisabled: {
		cursor: "default"
	}
});
