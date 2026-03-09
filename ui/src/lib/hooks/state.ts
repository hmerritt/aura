import { useStore as useStoreDefault } from "@tanstack/react-store";
import { type RootState, store } from "state";

/**
 * State selector hook.
 *
 * Uses main store defined in `state/index.ts`.
 *
 * @example
 * const count = useStore((state) => state.count.current);
 */
export const useStore = <TSelected>(
	selector: (state: RootState) => TSelected
): TSelected => useStoreDefault(store, selector);
