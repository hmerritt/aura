export type SplitScreenMode = "side-by-side" | "quad";

export type SplitSectorId =
    | "left"
    | "right"
    | "topLeft"
    | "topRight"
    | "bottomLeft"
    | "bottomRight";

export type SplitDividers = {
    xRatio: number;
    yRatio: number;
};

export type SplitAssignments = Partial<Record<SplitSectorId, string | null>>;

export const SIDE_BY_SIDE_SECTORS = [
    "left",
    "right"
] as const satisfies readonly SplitSectorId[];
export const QUAD_SECTORS = [
    "topLeft",
    "topRight",
    "bottomLeft",
    "bottomRight"
] as const satisfies readonly SplitSectorId[];
export const ALL_SECTORS = [
    ...SIDE_BY_SIDE_SECTORS,
    ...QUAD_SECTORS
] as const satisfies readonly SplitSectorId[];

const clamp01 = (value: number) => {
    if (!Number.isFinite(value)) return 0.5;
    if (value < 0) return 0;
    if (value > 1) return 1;
    return value;
};

export const getVisibleSectors = (mode: SplitScreenMode): readonly SplitSectorId[] =>
    mode === "quad" ? QUAD_SECTORS : SIDE_BY_SIDE_SECTORS;

export const normalizeDividers = (input?: Partial<SplitDividers>): SplitDividers => ({
    xRatio: clamp01(input?.xRatio ?? 0.5),
    yRatio: clamp01(input?.yRatio ?? 0.5)
});

export const clampDividers = (
    dividers: SplitDividers,
    rect: { width: number; height: number },
    minPaneSizePx: number
): SplitDividers => {
    const width = Math.max(0, rect.width);
    const height = Math.max(0, rect.height);
    const minPx = Number.isFinite(minPaneSizePx) ? Math.max(0, minPaneSizePx) : 0;

    const xMin = width > minPx * 2 && width > 0 ? minPx / width : 0.5;
    const yMin = height > minPx * 2 && height > 0 ? minPx / height : 0.5;

    return {
        xRatio: Math.min(1 - xMin, Math.max(xMin, clamp01(dividers.xRatio))),
        yRatio: Math.min(1 - yMin, Math.max(yMin, clamp01(dividers.yRatio)))
    };
};

const fillModeAssignments = (
    sectors: readonly SplitSectorId[],
    paneIds: readonly string[],
    defaults: SplitAssignments | undefined,
    assignments: SplitAssignments
) => {
    const validPaneIds = new Set(paneIds);
    const used = new Set<string>();

    for (const sector of sectors) {
        const paneId = defaults?.[sector];
        if (!paneId || !validPaneIds.has(paneId) || used.has(paneId)) {
            assignments[sector] = null;
            continue;
        }
        assignments[sector] = paneId;
        used.add(paneId);
    }

    for (const sector of sectors) {
        if (assignments[sector]) continue;
        const nextPane = paneIds.find((paneId) => !used.has(paneId));
        assignments[sector] = nextPane ?? null;
        if (nextPane) {
            used.add(nextPane);
        }
    }
};

export const normalizeAssignments = (
    paneIdsInput: readonly string[],
    defaults?: SplitAssignments
): SplitAssignments => {
    const paneIds = [...new Set(paneIdsInput.filter(Boolean))];
    const assignments: SplitAssignments = {};

    for (const sector of ALL_SECTORS) {
        assignments[sector] = null;
    }

    fillModeAssignments(SIDE_BY_SIDE_SECTORS, paneIds, defaults, assignments);
    fillModeAssignments(QUAD_SECTORS, paneIds, defaults, assignments);

    return assignments;
};

export const normalizeAssignmentsWithExisting = (
    paneIdsInput: readonly string[],
    existing: SplitAssignments
): SplitAssignments => normalizeAssignments(paneIdsInput, existing);

export const getAssignedPaneIdsForMode = (
    assignments: SplitAssignments,
    mode: SplitScreenMode
): Set<string> => {
    const visible = getVisibleSectors(mode);
    const result = new Set<string>();

    for (const sector of visible) {
        const paneId = assignments[sector];
        if (paneId) result.add(paneId);
    }

    return result;
};

export const findPaneSectorInMode = (
    assignments: SplitAssignments,
    mode: SplitScreenMode,
    paneId: string
): SplitSectorId | null => {
    for (const sector of getVisibleSectors(mode)) {
        if (assignments[sector] === paneId) {
            return sector;
        }
    }
    return null;
};

export type MovePaneInModeResult = {
    assignments: SplitAssignments;
    changed: boolean;
    from: SplitSectorId | "hidden";
    to: SplitSectorId;
};

export const movePaneInMode = (
    assignments: SplitAssignments,
    mode: SplitScreenMode,
    paneId: string,
    targetSector: SplitSectorId
): MovePaneInModeResult => {
    const visibleSectors = getVisibleSectors(mode);
    if (!visibleSectors.includes(targetSector)) {
        return {
            assignments,
            changed: false,
            from: "hidden",
            to: targetSector
        };
    }

    const currentSector = findPaneSectorInMode(assignments, mode, paneId);
    const currentTargetPane = assignments[targetSector] ?? null;

    if (currentSector === targetSector) {
        return {
            assignments,
            changed: false,
            from: currentSector ?? "hidden",
            to: targetSector
        };
    }

    const nextAssignments: SplitAssignments = { ...assignments };
    nextAssignments[targetSector] = paneId;

    if (currentSector) {
        nextAssignments[currentSector] = currentTargetPane;
    }

    return {
        assignments: nextAssignments,
        changed: true,
        from: currentSector ?? "hidden",
        to: targetSector
    };
};
