/// <reference types="vite/client" />
import type { AuraHostBridge } from "./host/types";

declare global {
    interface Window {
        __AURA_SETTINGS_HOST?: AuraHostBridge;
    }
}

export {};
