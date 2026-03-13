/// <reference types="vite/client" />
/// <reference types="vite/types/importMeta.d.ts" />
import type { AuraHostBridge } from "./host/types";

declare global {
    interface Window {
        __AURA_SETTINGS_HOST?: AuraHostBridge;
    }
}

export {};
