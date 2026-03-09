import * as stylex from "@stylexjs/stylex";
import { type AnimationParams, animate } from "animejs";
import {
	type ReactNode,
	type PointerEvent as ReactPointerEvent,
	createContext,
	useContext,
	useEffect,
	useLayoutEffect,
	useRef,
	useState
} from "react";
import { createPortal } from "react-dom";

import { type SxProp } from "lib/type-assertions";

import {
	type SplitAssignments,
	type SplitDividers,
	type SplitScreenMode,
	type SplitSectorId,
	clampDividers,
	findPaneSectorInMode,
	getAssignedPaneIdsForMode,
	getVisibleSectors,
	movePaneInMode,
	normalizeAssignments,
	normalizeDividers
} from "./layout";

export type { SplitScreenMode, SplitSectorId } from "./layout";

export type HiddenPaneBehavior = "preserve" | "unmount";

export type SplitScreenPaneDefinition = {
	id: string;
	node: ReactNode;
	label?: string;
};

export type SplitScreenLayoutState = {
	mode: SplitScreenMode;
	assignments: SplitAssignments;
	dividers: SplitDividers;
};

export type SplitScreenProps = React.JSX.IntrinsicElements["div"] &
	SxProp & {
		panes: SplitScreenPaneDefinition[];
		defaultMode?: SplitScreenMode;
		defaultAssignments?: SplitAssignments;
		defaultDividers?: Partial<SplitDividers>;
		hiddenPaneBehavior?: HiddenPaneBehavior;
		minPaneSizePx?: number;
		showModeControls?: boolean;
		onLayoutChange?: (state: SplitScreenLayoutState) => void;
		onModeChange?: (mode: SplitScreenMode) => void;
		onPaneMove?: (payload: {
			paneId: string;
			from: SplitSectorId | "hidden";
			to: SplitSectorId;
		}) => void;
	};

type DividerInteraction = {
	kind: "divider";
	axis: "x" | "y";
};

type PaneInteraction = {
	kind: "pane";
	paneId: string;
	originSector: SplitSectorId | "hidden";
};

type ActiveInteraction = DividerInteraction | PaneInteraction;

type SplitScreenHandleContextValue = {
	startPaneDragFromHandle: (
		paneId: string,
		evt: ReactPointerEvent<HTMLElement>
	) => void;
};

const clampPercent = (ratio: number) =>
	`${Math.min(100, Math.max(0, Math.round(ratio * 10000) / 100))}%`;

const SplitScreenHandleContext = createContext<SplitScreenHandleContextValue | null>(
	null
);

export type SplitScreenHandleProps = {
	onPointerDown: (evt: ReactPointerEvent<HTMLElement>) => void;
	draggable: false;
	role: "button";
	tabIndex: 0;
	"aria-label": string;
	"data-split-screen-handle": "true";
	"data-split-screen-handle-pane-id": string;
};

const shouldAnimateUi = () => {
	if (typeof window === "undefined") return false;
	if (env.isTest) return false;
	try {
		return !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
	} catch {
		return true;
	}
};

const safeAnimate = (
	targets: Element | Element[] | NodeListOf<Element>,
	props: AnimationParams
) => {
	if (!shouldAnimateUi()) return;
	try {
		animate(targets, props);
	} catch {
		// Animation failures should never break layout interactions.
	}
};

export const useSplitScreenHandle = (paneId: string): SplitScreenHandleProps => {
	const ctx = useContext(SplitScreenHandleContext);

	if (!ctx) {
		throw new Error(
			"useSplitScreenHandle must be used within <SplitScreen /> pane content."
		);
	}

	return {
		onPointerDown: (evt) => ctx.startPaneDragFromHandle(paneId, evt),
		draggable: false,
		role: "button",
		tabIndex: 0,
		"aria-label": `Move pane ${paneId}`,
		"data-split-screen-handle": "true",
		"data-split-screen-handle-pane-id": paneId
	};
};

const getOrCreatePaneHost = (
	paneIds: string[],
	paneHostMap: Map<string, HTMLDivElement>
) => {
	let changed = false;

	for (const paneId of paneIds) {
		const existingHost = paneHostMap.get(paneId);
		if (existingHost || typeof document === "undefined") continue;

		const host = document.createElement("div");
		host.dataset.splitScreenPaneHost = paneId;
		host.style.width = "100%";
		host.style.height = "100%";
		host.style.minWidth = "0";
		host.style.minHeight = "0";
		paneHostMap.set(paneId, host);
		changed = true;
	}

	return changed;
};

export const SplitScreen = ({
	panes,
	sx,
	style: _style,
	defaultMode = "side-by-side",
	defaultAssignments,
	defaultDividers,
	hiddenPaneBehavior = "preserve",
	minPaneSizePx = 120,
	showModeControls = false,
	onLayoutChange,
	onModeChange,
	onPaneMove,
	...divProps
}: SplitScreenProps) => {
	const uniquePanes: SplitScreenPaneDefinition[] = [];
	const seenPaneIds = new Set<string>();
	for (const pane of panes) {
		if (!pane?.id || seenPaneIds.has(pane.id)) continue;
		seenPaneIds.add(pane.id);
		uniquePanes.push(pane);
	}

	const paneIds = uniquePanes.map((pane) => pane.id);
	const paneIdsKey = paneIds.join("|");

	const [mode, setMode] = useState<SplitScreenMode>(defaultMode);
	const [assignmentsState, setAssignments] = useState<SplitAssignments>(() =>
		normalizeAssignments(paneIds, defaultAssignments)
	);
	const [dividers, setDividers] = useState<SplitDividers>(() =>
		normalizeDividers(defaultDividers)
	);
	const [activeInteraction, setActiveInteraction] = useState<ActiveInteraction | null>(
		null
	);
	const [hoverSector, setHoverSector] = useState<SplitSectorId | null>(null);
	const [paneHosts, setPaneHosts] = useState<Record<string, HTMLDivElement>>({});

	const rootRef = useRef<HTMLDivElement | null>(null);
	const hiddenDockRef = useRef<HTMLDivElement | null>(null);
	const verticalDividerRef = useRef<HTMLDivElement | null>(null);
	const horizontalDividerRef = useRef<HTMLDivElement | null>(null);
	const sectorRefs = useRef<Partial<Record<SplitSectorId, HTMLDivElement | null>>>({});
	const paneHostMapRef = useRef<Map<string, HTMLDivElement>>(new Map());
	const assignments = normalizeAssignments(paneIds, assignmentsState);
	const assignmentsRef = useRef(assignments);
	const modeRef = useRef(mode);
	const hoverSectorRef = useRef<SplitSectorId | null>(hoverSector);
	const layoutDidMountRef = useRef(false);
	const previousModeRef = useRef(mode);
	const activeInteractionRef = useRef<ActiveInteraction | null>(activeInteraction);
	const visibleSectors = getVisibleSectors(mode);
	const visiblePaneIds = getAssignedPaneIdsForMode(assignments, mode);

	useEffect(() => {
		assignmentsRef.current = assignments;
	}, [assignments]);

	useEffect(() => {
		modeRef.current = mode;
	}, [mode]);

	useEffect(() => {
		hoverSectorRef.current = hoverSector;
	}, [hoverSector]);

	useEffect(() => {
		activeInteractionRef.current = activeInteraction;
	}, [activeInteraction]);

	useLayoutEffect(() => {
		const changed = getOrCreatePaneHost(paneIds, paneHostMapRef.current);
		if (!changed) return;

		setPaneHosts(Object.fromEntries(paneHostMapRef.current.entries()));
	}, [paneIds, paneIdsKey]);

	useEffect(() => {
		const visiblePaneIdSet = new Set(paneIds);
		let changed = false;
		for (const [paneId, host] of paneHostMapRef.current.entries()) {
			if (visiblePaneIdSet.has(paneId)) continue;
			host.remove();
			paneHostMapRef.current.delete(paneId);
			changed = true;
		}

		if (changed) {
			setPaneHosts(Object.fromEntries(paneHostMapRef.current.entries()));
		}
	}, [paneIds, paneIdsKey]);

	useEffect(() => {
		const hostMap = paneHostMapRef.current;
		return () => {
			for (const host of hostMap.values()) {
				host.remove();
			}
			hostMap.clear();
		};
	}, []);

	useLayoutEffect(() => {
		for (const paneId of paneIds) {
			const host = paneHostMapRef.current.get(paneId);
			if (!host) continue;

			const sector = findPaneSectorInMode(assignments, mode, paneId);
			if (sector) {
				const sectorNode = sectorRefs.current[sector];
				if (sectorNode && host.parentElement !== sectorNode) {
					sectorNode.appendChild(host);
				}
				continue;
			}

			if (hiddenPaneBehavior === "preserve") {
				const hiddenDock = hiddenDockRef.current;
				if (hiddenDock && host.parentElement !== hiddenDock) {
					hiddenDock.appendChild(host);
				}
				continue;
			}

			if (host.parentElement) {
				host.parentElement.removeChild(host);
			}
		}
	}, [assignments, hiddenPaneBehavior, mode, paneIds, paneIdsKey]);

	useEffect(() => {
		if (!layoutDidMountRef.current) {
			layoutDidMountRef.current = true;
			return;
		}

		onLayoutChange?.({
			mode,
			assignments,
			dividers
		});
	}, [assignments, dividers, mode, onLayoutChange]);

	useEffect(() => {
		if (previousModeRef.current === mode) return;

		const targets = [
			...visibleSectors
				.map((sector) => sectorRefs.current[sector])
				.filter((node): node is HTMLDivElement => Boolean(node))
		];
		if (verticalDividerRef.current) targets.push(verticalDividerRef.current);
		if (mode === "quad" && horizontalDividerRef.current) {
			targets.push(horizontalDividerRef.current);
		}

		safeAnimate(targets, {
			opacity: [0.7, 1],
			scale: [0.985, 1],
			duration: 180
		});

		previousModeRef.current = mode;
	}, [mode, visibleSectors]);

	useEffect(() => {
		if (!activeInteraction) return;

		const previousUserSelect = document.body.style.userSelect;
		const previousCursor = document.body.style.cursor;
		document.body.style.userSelect = "none";
		document.body.style.cursor =
			activeInteraction.kind === "divider"
				? activeInteraction.axis === "x"
					? "col-resize"
					: "row-resize"
				: "grabbing";

		const getRootRect = () => rootRef.current?.getBoundingClientRect() ?? null;
		const getHoveredSectorAtPoint = (clientX: number, clientY: number) => {
			const currentMode = modeRef.current;
			for (const sector of getVisibleSectors(currentMode)) {
				const node = sectorRefs.current[sector];
				if (!node) continue;
				const rect = node.getBoundingClientRect();
				if (
					clientX >= rect.left &&
					clientX <= rect.right &&
					clientY >= rect.top &&
					clientY <= rect.bottom
				) {
					return sector;
				}
			}
			return null;
		};

		const onPointerMove = (evt: PointerEvent) => {
			const currentInteraction = activeInteractionRef.current;
			if (!currentInteraction) return;

			if (currentInteraction.kind === "divider") {
				const rootRect = getRootRect();
				if (!rootRect) return;

				if (currentInteraction.axis === "x") {
					const rawRatio =
						(evt.clientX - rootRect.left) / Math.max(1, rootRect.width);
					setDividers((prev) =>
						clampDividers(
							{ ...prev, xRatio: rawRatio },
							{ width: rootRect.width, height: rootRect.height },
							minPaneSizePx
						)
					);
					return;
				}

				const rawRatio =
					(evt.clientY - rootRect.top) / Math.max(1, rootRect.height);
				setDividers((prev) =>
					clampDividers(
						{ ...prev, yRatio: rawRatio },
						{ width: rootRect.width, height: rootRect.height },
						minPaneSizePx
					)
				);
				return;
			}

			const hovered = getHoveredSectorAtPoint(evt.clientX, evt.clientY);
			setHoverSector(hovered);
		};

		const onPointerEnd = (evt: PointerEvent) => {
			const currentInteraction = activeInteractionRef.current;
			if (!currentInteraction) return;

			if (currentInteraction.kind === "pane") {
				const dropSector =
					hoverSectorRef.current ??
					getHoveredSectorAtPoint(evt.clientX, evt.clientY);

				if (dropSector) {
					const result = movePaneInMode(
						assignmentsRef.current,
						modeRef.current,
						currentInteraction.paneId,
						dropSector
					);

					if (result.changed) {
						setAssignments(result.assignments);
						onPaneMove?.({
							paneId: currentInteraction.paneId,
							from: result.from,
							to: result.to
						});

						const targetNode = sectorRefs.current[dropSector];
						if (targetNode) {
							safeAnimate(targetNode, {
								scale: [0.99, 1],
								opacity: [0.85, 1],
								duration: 160
							});
						}
					}
				}
			}

			setHoverSector(null);
			setActiveInteraction(null);
		};

		window.addEventListener("pointermove", onPointerMove);
		window.addEventListener("pointerup", onPointerEnd);
		window.addEventListener("pointercancel", onPointerEnd);

		return () => {
			window.removeEventListener("pointermove", onPointerMove);
			window.removeEventListener("pointerup", onPointerEnd);
			window.removeEventListener("pointercancel", onPointerEnd);
			document.body.style.userSelect = previousUserSelect;
			document.body.style.cursor = previousCursor;
		};
	}, [activeInteraction, minPaneSizePx, onPaneMove]);

	const setSectorNode = (sector: SplitSectorId) => (node: HTMLDivElement | null) => {
		sectorRefs.current[sector] = node;
	};

	const setHiddenDockNode = (node: HTMLDivElement | null) => {
		hiddenDockRef.current = node;
	};

	const setRootNode = (node: HTMLDivElement | null) => {
		rootRef.current = node;
	};

	const startDividerDrag = (
		axis: "x" | "y",
		evt: ReactPointerEvent<HTMLDivElement>
	) => {
		evt.preventDefault();
		setActiveInteraction({ kind: "divider", axis });
		setHoverSector(null);
	};

	const startPaneDrag = (paneId: string, evt: ReactPointerEvent<HTMLElement>) => {
		if (!paneHostMapRef.current.has(paneId)) {
			return;
		}

		evt.preventDefault();
		evt.stopPropagation();
		const originSector = findPaneSectorInMode(
			assignmentsRef.current,
			modeRef.current,
			paneId
		);
		setHoverSector(originSector);
		setActiveInteraction({
			kind: "pane",
			paneId,
			originSector: originSector ?? "hidden"
		});
	};

	const handleModeChange = (nextMode: SplitScreenMode) => {
		if (nextMode === mode) return;
		setMode(nextMode);
		onModeChange?.(nextMode);
		setHoverSector(null);
	};

	const isDividerDragging = activeInteraction?.kind === "divider";
	const handleContextValue: SplitScreenHandleContextValue = {
		startPaneDragFromHandle: startPaneDrag
	};

	return (
		<div
			{...divProps}
			{...stylex.props(
				styles.root,
				styles.splitRatios(
					clampPercent(dividers.xRatio),
					clampPercent(dividers.yRatio)
				),
				sx
			)}
			data-split-screen
			data-split-screen-mode={mode}
			data-split-screen-dragging-divider={isDividerDragging ? "true" : "false"}
			ref={setRootNode}
		>
			{showModeControls ? (
				<div
					{...stylex.props(styles.controls)}
					data-testid="split-screen-mode-controls"
				>
					<button
						{...stylex.props(
							styles.controlButton,
							mode === "side-by-side" && styles.controlButtonActive
						)}
						aria-pressed={mode === "side-by-side"}
						data-testid="split-screen-mode-side-by-side"
						onClick={() => handleModeChange("side-by-side")}
						type="button"
					>
						Side by Side
					</button>
					<button
						{...stylex.props(
							styles.controlButton,
							mode === "quad" && styles.controlButtonActive
						)}
						aria-pressed={mode === "quad"}
						data-testid="split-screen-mode-quad"
						onClick={() => handleModeChange("quad")}
						type="button"
					>
						Quad
					</button>
				</div>
			) : null}

			<div {...stylex.props(styles.stage)} data-testid="split-screen-stage">
				{visibleSectors.map((sector) => {
					const isDropTarget = hoverSector === sector;

					return (
						<div
							key={sector}
							{...stylex.props(
								styles.sector,
								mode === "side-by-side"
									? sector === "left"
										? styles.sideLeft
										: styles.sideRight
									: sector === "topLeft"
										? styles.quadTopLeft
										: sector === "topRight"
											? styles.quadTopRight
											: sector === "bottomLeft"
												? styles.quadBottomLeft
												: styles.quadBottomRight,
								isDropTarget && styles.sectorDropTarget
							)}
							aria-label={`Split sector ${sector}`}
							data-testid={`split-screen-sector-${sector}`}
							data-split-screen-sector={sector}
							ref={setSectorNode(sector)}
						/>
					);
				})}

				<div
					{...stylex.props(styles.verticalDivider)}
					aria-label="Resize vertical split"
					data-testid="split-screen-divider-vertical"
					onPointerDown={(evt) => startDividerDrag("x", evt)}
					ref={verticalDividerRef}
					role="separator"
				>
					<div {...stylex.props(styles.dividerLine)} />
				</div>

				{mode === "quad" ? (
					<div
						{...stylex.props(styles.horizontalDivider)}
						aria-label="Resize horizontal split"
						data-testid="split-screen-divider-horizontal"
						onPointerDown={(evt) => startDividerDrag("y", evt)}
						ref={horizontalDividerRef}
						role="separator"
					>
						<div {...stylex.props(styles.dividerLineHorizontal)} />
					</div>
				) : null}
			</div>

			<div
				{...stylex.props(styles.hiddenDock)}
				aria-hidden="true"
				data-testid="split-screen-hidden-dock"
				ref={setHiddenDockNode}
			/>

			{uniquePanes.map((pane) => {
				const paneHost = paneHosts[pane.id];
				if (!paneHost) return null;

				const isVisible = visiblePaneIds.has(pane.id);
				if (!isVisible && hiddenPaneBehavior === "unmount") {
					return null;
				}

				return createPortal(
					<SplitScreenHandleContext.Provider value={handleContextValue}>
						{pane.node}
					</SplitScreenHandleContext.Provider>,
					paneHost,
					pane.id
				);
			})}
		</div>
	);
};

const styles = stylex.create({
	root: {
		display: "flex",
		flexDirection: "column",
		minWidth: 0,
		position: "relative",
		width: "100%"
	},
	splitRatios: (xPct: string, yPct: string) => ({
		"--split-x-pct": xPct,
		"--split-y-pct": yPct
	}),
	controls: {
		display: "flex",
		justifyContent: "flex-end"
	},
	controlButton: {
		backgroundColor: "rgba(255, 255, 255, 0.06)",
		borderColor: "rgba(255, 255, 255, 0.14)",
		borderRadius: "10px",
		borderStyle: "solid",
		borderWidth: "1px",
		color: "rgb(232, 236, 243)",
		cursor: "pointer",
		fontFamily: "monospace",
		fontSize: "0.8rem",
		height: "28px",
		minWidth: "88px"
	},
	controlButtonActive: {
		backgroundColor: "rgba(120, 170, 255, 0.18)",
		borderColor: "rgba(120, 170, 255, 0.5)"
	},
	stage: {
		flex: "1 1 auto",
		minHeight: "260px",
		overflow: "hidden",
		position: "relative"
	},
	sector: {
		minWidth: 0,
		minHeight: 0,
		overflow: "hidden",
		position: "absolute"
	},
	sectorDropTarget: {
		borderColor: "rgba(140, 190, 255, 0.65)",
		boxShadow: "inset 0 0 0 1px rgba(140, 190, 255, 0.25)"
	},
	sideLeft: {
		bottom: 0,
		left: 0,
		right: "calc(100% - var(--split-x-pct) + 6px)",
		top: 0
	},
	sideRight: {
		bottom: 0,
		left: "calc(var(--split-x-pct) + 6px)",
		right: 0,
		top: 0
	},
	quadTopLeft: {
		bottom: "calc(100% - var(--split-y-pct) + 6px)",
		left: 0,
		right: "calc(100% - var(--split-x-pct) + 6px)",
		top: 0
	},
	quadTopRight: {
		bottom: "calc(100% - var(--split-y-pct) + 6px)",
		left: "calc(var(--split-x-pct) + 6px)",
		right: 0,
		top: 0
	},
	quadBottomLeft: {
		bottom: 0,
		left: 0,
		right: "calc(100% - var(--split-x-pct) + 6px)",
		top: "calc(var(--split-y-pct) + 6px)"
	},
	quadBottomRight: {
		bottom: 0,
		left: "calc(var(--split-x-pct) + 6px)",
		right: 0,
		top: "calc(var(--split-y-pct) + 6px)"
	},
	verticalDivider: {
		alignItems: "center",
		bottom: 0,
		cursor: "col-resize",
		display: "flex",
		justifyContent: "center",
		left: "calc(var(--split-x-pct) - 6px)",
		position: "absolute",
		top: 0,
		width: "12px",
		zIndex: 3
	},
	horizontalDivider: {
		alignItems: "center",
		cursor: "row-resize",
		display: "flex",
		height: "12px",
		justifyContent: "center",
		left: 0,
		position: "absolute",
		right: 0,
		top: "calc(var(--split-y-pct) - 6px)",
		zIndex: 3
	},
	dividerLine: {
		backgroundColor: "rgba(255, 255, 255, 0.22)",
		borderRadius: "999px",
		height: "100%",
		width: "2px"
	},
	dividerLineHorizontal: {
		backgroundColor: "rgba(255, 255, 255, 0.22)",
		borderRadius: "999px",
		height: "2px",
		width: "100%"
	},
	hiddenDock: {
		height: 0,
		left: "-10000px",
		overflow: "hidden",
		position: "absolute",
		top: "-10000px",
		width: 0
	}
});
