export type RendererMode = "image" | "shader";
export type OutputFormat = "jpg" | "png";
export type ShaderDesktopScope = "virtual" | "primary";
export type ShaderColorSpace = "unorm" | "srgb";

export interface ConfigWarning {
    key_path: string;
    issue: string;
    fallback: string;
    raw_value?: string | null;
}

export type SettingsSourceConfig =
    | {
          type: "file";
          path: string;
      }
    | {
          type: "directory";
          path: string;
          recursive: boolean;
          extensions?: string[] | null;
      }
    | {
          type: "rss";
          url: string;
          max_items: number;
          download_dir?: string | null;
      };

export interface SettingsDocument {
    renderer: RendererMode;
    image: {
        timer: string;
        remoteUpdateTimer: string;
        sources: SettingsSourceConfig[];
        format: OutputFormat;
        jpeg_quality: number;
    };
    shader: {
        name: string;
        target_fps: number;
        resolution: number;
        mouse_enabled: boolean;
        desktop_scope: ShaderDesktopScope;
        color_space: ShaderColorSpace;
    };
    updater: {
        enabled: boolean;
        checkInterval: string;
        feedUrl: string;
    };
    cache_dir: string;
    state_file: string;
    log_level: string;
    max_cache_mb: number;
    max_cache_age_days: number;
}

export interface SettingsLoadResult {
    document: SettingsDocument;
    warnings: ConfigWarning[];
    imagePreview: {
        currentId: string | null;
        currentSrc: string | null;
        nextId: string | null;
        nextSrc: string | null;
    };
    previewFrame: {
        width: number;
        height: number;
    };
}

export interface SettingsValidationResult {
    warnings: ConfigWarning[];
}

export interface BootstrapPayload {
    version: string;
    configPath: string;
    devServerEnv: string;
}

export interface SaveSettingsPayload {
    result: SettingsLoadResult;
    restartRequested: boolean;
}

export interface SaveSettingsRequest {
    document: SettingsDocument;
    lockedImageId: string | null;
}

export interface AuraHostBridge {
    onMessage(callback: (message: string) => void): void;
    post(message: unknown): void;
}

export type HostCommand =
    | "bootstrap"
    | "load_settings"
    | "validate_settings"
    | "save_settings";

export type HostResponseMap = {
    bootstrap: BootstrapPayload;
    load_settings: SettingsLoadResult;
    validate_settings: SettingsValidationResult;
    save_settings: SaveSettingsPayload;
};

export type HostRequestMap = {
    bootstrap: Record<string, never>;
    load_settings: Record<string, never>;
    validate_settings: SettingsDocument;
    save_settings: SaveSettingsRequest;
    close_window: Record<string, never>;
};

export interface HostResponseEnvelope<
    TCommand extends string = string,
    TPayload = unknown
> {
    id?: string | null;
    ok: boolean;
    command: TCommand;
    payload: TPayload;
    error?: string | null;
}
