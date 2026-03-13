import { RootState, store, updateSlice, updateState } from "state";
import { beforeEach, describe, expect, it } from "vitest";

import { colorStore } from "state/slices/color/colorStore";
import { countStore } from "state/slices/count/countStore";
import { settingsStore } from "state/slices/settings/settingsStore";
import {
    createInitialState,
    normalizeRootState,
    partializePersistedState
} from "./store";

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

describe("partializePersistedState", () => {
    it("excludes the settings slice from persistence", () => {
        const persistedState = partializePersistedState(createInitialState());

        expect(persistedState).toEqual({
            color: { ...colorStore, colors: [...colorStore.colors] },
            count: { ...countStore }
        });
    });
});

describe("normalizeRootState", () => {
    it("restores the settings slice when it is missing", () => {
        const normalizedState = normalizeRootState({
            color: { ...colorStore, colors: [...colorStore.colors] },
            count: { ...countStore }
        });

        expect(normalizedState.settings).toEqual({ ...settingsStore });
    });

    it("allows updateSlice to recover from a partial root state", () => {
        store.setState(
            () =>
                ({
                    color: { ...colorStore, colors: [...colorStore.colors] },
                    count: { ...countStore }
                }) as RootState
        );

        updateSlice("settings", (settings) => {
            settings.status = "loading";
        });

        expect(store.state.settings.status).toBe("loading");
    });
});
