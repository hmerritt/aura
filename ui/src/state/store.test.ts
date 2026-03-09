import { RootState, store, updateSlice, updateState } from "state";
import { beforeEach, describe, expect, it } from "vitest";

import { colorStore } from "state/slices/color/colorStore";
import { countStore } from "state/slices/count/countStore";

const createInitialState = (): RootState => ({
    color: { ...colorStore, colors: [...colorStore.colors] },
    count: { ...countStore }
});

const resetStore = (): RootState => {
    const initialState = createInitialState();
    store.setState(() => initialState);
    return initialState;
};

describe("updateState", () => {
    beforeEach(() => {
        resetStore();
    });

    it("updates state via a shallow root copy", () => {
        const prevState = store.state;
        const prevCount = prevState.count;
        const prevColor = prevState.color;
        let draftRef: RootState | null = null;

        updateState((draft) => {
            draftRef = draft;
            draft.count.current = 5;
            draft.color.current = draft.color.colors[1];
        });

        const nextState = store.state;

        expect(nextState).not.toBe(prevState);
        expect(nextState).toBe(draftRef as unknown as RootState);
        expect(nextState.count).toBe(prevCount);
        expect(nextState.color).toBe(prevColor);
        expect(prevCount.current).toBe(5);
        expect(nextState.count.current).toBe(5);
        expect(nextState.color.current).toBe(prevColor.colors[1]);
    });
});

describe("updateSlice", () => {
    beforeEach(() => {
        resetStore();
    });

    it("updates only the target slice", () => {
        const prevState = store.state;
        const prevCount = prevState.count;
        const prevColor = prevState.color;
        let draftRef: RootState["count"] | null = null;

        updateSlice("count", (count) => {
            draftRef = count;
            count.current = 7;
        });

        const nextState = store.state;

        expect(nextState).not.toBe(prevState);
        expect(nextState.count).toBe(draftRef as unknown as RootState["count"]);
        expect(nextState.count).not.toBe(prevCount);
        expect(nextState.color).toBe(prevColor);
        expect(prevCount.current).toBe(0);
        expect(nextState.count.current).toBe(7);
    });
});
