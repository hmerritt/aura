import type {
  AuraHostBridge,
  BootstrapPayload,
  HostCommand,
  HostRequestMap,
  HostResponseEnvelope,
  HostResponseMap,
  SaveSettingsPayload,
  SaveSettingsRequest,
  SettingsDocument,
  SettingsLoadResult,
  SettingsValidationResult,
} from "./types";

type PendingRequest = {
  command: HostCommand;
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
};

type MockState = {
  document: SettingsDocument;
};

const MOCK_DOCUMENT: SettingsDocument = {
  renderer: "image",
  image: {
    timer: "30m",
    remoteUpdateTimer: "4h",
    sources: [
      {
        type: "directory",
        path: "C:/Users/you/Pictures",
        recursive: true,
        extensions: ["jpg", "png", "webp"],
      },
    ],
    format: "jpg",
    jpeg_quality: 90,
  },
  shader: {
    name: "gradient_glossy",
    target_fps: 60,
    resolution: 100,
    mouse_enabled: false,
    desktop_scope: "virtual",
    color_space: "unorm",
  },
  updater: {
    enabled: true,
    checkInterval: "6h",
    feedUrl: "https://github.com/hmerritt/aura/releases/latest/download",
  },
  cache_dir: "C:/Users/you/AppData/Local/aura/cache",
  state_file: "C:/Users/you/AppData/Local/aura/state.json",
  log_level: "info",
  max_cache_mb: 1024,
  max_cache_age_days: 30,
};

class HostRequestError extends Error {
  readonly payload: unknown;

  constructor(message: string, payload: unknown) {
    super(message);
    this.name = "HostRequestError";
    this.payload = payload;
  }
}

function cloneMockDocument(): SettingsDocument {
  return JSON.parse(JSON.stringify(MOCK_DOCUMENT)) as SettingsDocument;
}

function createMockBridge(): AuraHostBridge {
  const listeners: Array<(message: string) => void> = [];
  const state: MockState = {
    document: cloneMockDocument(),
  };

  const emit = (envelope: HostResponseEnvelope) => {
    const raw = JSON.stringify(envelope);
    window.setTimeout(() => {
      for (const listener of listeners) {
        listener(raw);
      }
    }, 80);
  };

  return {
    onMessage(callback) {
      listeners.push(callback);
    },
    post(message) {
      const request = typeof message === "string" ? JSON.parse(message) : message;
      const id = request.id as string | undefined;
      const command = request.command as keyof HostRequestMap;
      const payload = request.payload;

      if (command === "close_window") {
        return;
      }

      switch (command) {
        case "bootstrap":
          emit({
            id,
            ok: true,
            command,
            payload: {
              version: "dev-preview",
              configPath: "mock://aura.hcl",
              devServerEnv: "AURA_SETTINGS_UI_DEV_URL",
            } satisfies BootstrapPayload,
          });
          break;
        case "load_settings":
          emit({
            id,
            ok: true,
            command,
            payload: {
              document: state.document,
              warnings: [],
              imagePreview: {
                currentId: "mock-current",
                currentSrc: null,
                nextId: "mock-next",
                nextSrc: null,
              },
              previewFrame: {
                width: 1920,
                height: 1080,
              },
            } satisfies SettingsLoadResult,
          });
          break;
        case "validate_settings":
          emit({
            id,
            ok: true,
            command,
            payload: {
              warnings: [],
            } satisfies SettingsValidationResult,
          });
          break;
        case "save_settings":
          state.document = (payload as SaveSettingsRequest).document;
          emit({
            id,
            ok: true,
            command,
            payload: {
              result: {
                document: state.document,
                warnings: [],
                imagePreview: {
                  currentId: "mock-current",
                  currentSrc: null,
                  nextId: "mock-next",
                  nextSrc: null,
                },
                previewFrame: {
                  width: 1920,
                  height: 1080,
                },
              },
              restartRequested: false,
            } satisfies SaveSettingsPayload,
          });
          break;
        default:
          emit({
            id,
            ok: false,
            command,
            payload: {},
            error: `Unsupported mock command: ${String(command)}`,
          });
      }
    },
  };
}

function resolveHostBridge(): { bridge: AuraHostBridge; mode: "native" | "mock" } {
  if (window.__AURA_SETTINGS_HOST) {
    return { bridge: window.__AURA_SETTINGS_HOST, mode: "native" };
  }

  return { bridge: createMockBridge(), mode: "mock" };
}

class AuraSettingsHostClient {
  readonly mode: "native" | "mock";
  private readonly bridge: AuraHostBridge;
  private readonly pending = new Map<string, PendingRequest>();
  private nextId = 0;

  constructor() {
    const resolved = resolveHostBridge();
    this.bridge = resolved.bridge;
    this.mode = resolved.mode;
    this.bridge.onMessage((message) => {
      this.handleMessage(message);
    });
  }

  request<TCommand extends HostCommand>(
    command: TCommand,
    payload: HostRequestMap[TCommand],
  ): Promise<HostResponseMap[TCommand]> {
    const id = `${command}-${Date.now()}-${this.nextId++}`;
    this.bridge.post({
      id,
      command,
      payload,
    });

    return new Promise<HostResponseMap[TCommand]>((resolve, reject) => {
      this.pending.set(id, {
        command,
        resolve: (value) => resolve(value as HostResponseMap[TCommand]),
        reject,
      });
    });
  }

  send(command: "close_window", payload: HostRequestMap["close_window"]): void {
    this.bridge.post({
      id: undefined,
      command,
      payload,
    });
  }

  private handleMessage(raw: string): void {
    let envelope: HostResponseEnvelope;
    try {
      envelope = JSON.parse(raw) as HostResponseEnvelope;
    } catch (error) {
      console.error("Failed to parse Aura host response", error, raw);
      return;
    }

    if (!envelope.id) {
      return;
    }

    const pending = this.pending.get(envelope.id);
    if (!pending) {
      return;
    }
    this.pending.delete(envelope.id);

    if (!envelope.ok) {
      pending.reject(
        new HostRequestError(
          envelope.error ?? `Host command failed: ${pending.command}`,
          envelope.payload,
        ),
      );
      return;
    }

    pending.resolve(envelope.payload);
  }
}

export { HostRequestError };
export const auraSettingsHost = new AuraSettingsHostClient();
