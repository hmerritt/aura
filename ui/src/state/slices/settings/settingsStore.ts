import type { SettingsLoadResult } from "lib/host/types";

import type { LockedImageSelection, SettingsStatus } from "./settingsShared";

export interface ISettingsStore {
    status: SettingsStatus;
    result: SettingsLoadResult | null;
    lockedImageSelection: LockedImageSelection | null;
}

export const settingsStore: ISettingsStore = {
    status: "idle",
    result: null,
    lockedImageSelection: null
};

export default settingsStore;
