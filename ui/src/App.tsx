import { startTransition, useEffect, useState } from "react";
import { HostRequestError, auraSettingsHost } from "./host/client";
import type {
  BootstrapPayload,
  ConfigWarning,
  SettingsLoadResult,
  SettingsValidationResult,
} from "./host/types";

function formatWarnings(warnings: ConfigWarning[]): string {
  if (warnings.length === 0) {
    return "No warnings";
  }

  return warnings.map((warning) => `${warning.key_path}: ${warning.issue}`).join("\n");
}

export default function App() {
  const [bootstrap, setBootstrap] = useState<BootstrapPayload | null>(null);
  const [snapshot, setSnapshot] = useState<SettingsLoadResult | null>(null);
  const [validation, setValidation] = useState<SettingsValidationResult | null>(null);
  const [status, setStatus] = useState("Connecting to Aura host...");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function loadInitialState() {
      setBusy(true);
      setStatus(
        auraSettingsHost.mode === "mock"
          ? "Running in Bun/Vite preview mode with a mock Aura host."
          : "Connecting to Aura settings host...",
      );

      try {
        const [bootstrapPayload, settingsPayload] = await Promise.all([
          auraSettingsHost.request("bootstrap", {}),
          auraSettingsHost.request("load_settings", {}),
        ]);
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setBootstrap(bootstrapPayload);
          setSnapshot(settingsPayload);
          setValidation(null);
          setStatus("Settings bundle loaded. The real editor can build on this shell.");
        });
      } catch (error) {
        if (cancelled) {
          return;
        }
        setStatus(
          error instanceof Error ? error.message : "Failed to load settings bundle.",
        );
      } finally {
        if (!cancelled) {
          setBusy(false);
        }
      }
    }

    loadInitialState();
    return () => {
      cancelled = true;
    };
  }, []);

  async function reloadSnapshot() {
    setBusy(true);
    try {
      const next = await auraSettingsHost.request("load_settings", {});
      startTransition(() => {
        setSnapshot(next);
        setValidation(null);
        setStatus("Reloaded the current settings snapshot from Aura.");
      });
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to reload settings.");
    } finally {
      setBusy(false);
    }
  }

  async function validateSnapshot() {
    if (!snapshot) {
      return;
    }

    setBusy(true);
    try {
      const next = await auraSettingsHost.request(
        "validate_settings",
        snapshot.document,
      );
      startTransition(() => {
        setValidation(next);
        setStatus(
          next.warnings.length === 0
            ? "Validation passed with no warnings."
            : `Validation finished with ${next.warnings.length} warning(s).`,
        );
      });
    } catch (error) {
      if (error instanceof HostRequestError) {
        setStatus(error.message);
        return;
      }
      setStatus(error instanceof Error ? error.message : "Validation failed.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Aura Settings Host</p>
          <h1>React/Vite bundle is mounted</h1>
          <p className="lede">
            This is the bundle-backed shell for the settings window. The host
            bridge, bundle embedding, and typed IPC are live.
          </p>
        </div>
        <div className="badge-stack">
          <span className="badge">{auraSettingsHost.mode}</span>
          <span className="badge badge-quiet">
            {busy ? "working" : "idle"}
          </span>
        </div>
      </section>

      <section className="grid">
        <article className="panel panel-focus">
          <header className="panel-head">
            <div>
              <p className="panel-label">Runtime</p>
              <h2>Bundle handshake</h2>
            </div>
            <div className="actions">
              <button type="button" onClick={reloadSnapshot} disabled={busy}>
                Reload snapshot
              </button>
              <button type="button" onClick={validateSnapshot} disabled={busy || !snapshot}>
                Validate snapshot
              </button>
              <button
                type="button"
                className="ghost"
                onClick={() => auraSettingsHost.send("close_window", {})}
              >
                Close
              </button>
            </div>
          </header>

          <dl className="facts">
            <div>
              <dt>Version</dt>
              <dd>{bootstrap?.version ?? "waiting"}</dd>
            </div>
            <div>
              <dt>Config Path</dt>
              <dd>{bootstrap?.configPath ?? "waiting"}</dd>
            </div>
            <div>
              <dt>Dev Override</dt>
              <dd>{bootstrap?.devServerEnv ?? "waiting"}</dd>
            </div>
          </dl>

          <div className="status-strip">{status}</div>
        </article>

        <article className="panel">
          <header className="panel-head">
            <div>
              <p className="panel-label">Warnings</p>
              <h2>Config diagnostics</h2>
            </div>
          </header>
          <div className="warning-grid">
            <section>
              <h3>Load</h3>
              <pre>{formatWarnings(snapshot?.warnings ?? [])}</pre>
            </section>
            <section>
              <h3>Validate</h3>
              <pre>{formatWarnings(validation?.warnings ?? [])}</pre>
            </section>
          </div>
        </article>

        <article className="panel panel-wide">
          <header className="panel-head">
            <div>
              <p className="panel-label">Snapshot</p>
              <h2>Settings document preview</h2>
            </div>
          </header>
          <pre>
            {snapshot
              ? JSON.stringify(snapshot.document, null, 2)
              : "Waiting for the first settings payload..."}
          </pre>
        </article>
      </section>
    </main>
  );
}
