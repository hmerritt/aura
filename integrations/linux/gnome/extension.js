import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const AURA_BUS = 'io.github.hmerritt.Aura';
const COMPANION_BUS = 'io.github.hmerritt.Aura.Gnome';
const AURA_PATH = '/io/github/hmerritt/Aura';

const AuraXml = `<node>
  <interface name="io.github.hmerritt.Aura1">
    <method name="GetSnapshot"><arg type="s" direction="out"/></method>
    <method name="NextBackground"/>
    <method name="ReloadSettings"/>
    <method name="OpenSettings"/>
    <method name="Exit"/>
    <method name="ReportRendererStatus">
      <arg type="t" direction="in"/>
      <arg type="s" direction="in"/>
      <arg type="s" direction="in"/>
    </method>
    <signal name="SnapshotChanged"><arg type="s"/></signal>
  </interface>
</node>`;

const AuraProxy = Gio.DBusProxy.makeProxyWrapper(AuraXml);

const AuraShaderEffect = GObject.registerClass(
class AuraShaderEffect extends Shell.GLSLEffect {
    _init(core, colorSpace) {
        super._init();
        this._core = core;
        this._colorSpace = colorSpace;
        this._time = this.get_uniform_location('aura_time_seconds');
        this._frame = this.get_uniform_location('aura_frame_index');
        this._mouseEnabled = this.get_uniform_location('aura_mouse_enabled');
        this._resolution = this.get_uniform_location('aura_resolution');
        this._mouse = this.get_uniform_location('aura_mouse');
        this._srgb = this.get_uniform_location('aura_color_space_srgb');
    }

    vfunc_build_pipeline() {
        const declarations = `
uniform float aura_time_seconds;
uniform float aura_frame_index;
uniform float aura_mouse_enabled;
uniform vec4 aura_resolution;
uniform vec4 aura_mouse;
uniform float aura_color_space_srgb;
${this._core}
vec3 aura_linear_to_srgb(vec3 value) {
    vec3 low = value * 12.92;
    vec3 high = 1.055 * pow(max(value, vec3(0.0)), vec3(1.0 / 2.4)) - vec3(0.055);
    return vec3(
        value.r <= 0.0031308 ? low.r : high.r,
        value.g <= 0.0031308 ? low.g : high.g,
        value.b <= 0.0031308 ? low.b : high.b
    );
}`;
        const code = `
vec2 aura_size = max(aura_resolution.xy, vec2(1.0));
vec2 aura_coord = floor(cogl_tex_coord_in[0].xy * aura_size) + vec2(0.5);
AuraUniforms aura_values = AuraUniforms(
    aura_time_seconds,
    max(aura_frame_index, 0.0),
    max(aura_mouse_enabled, 0.0),
    0.0,
    aura_resolution,
    aura_mouse
);
vec4 aura_result = aura_main(aura_coord, aura_values);
if (aura_color_space_srgb > 0.5)
    aura_result.rgb = aura_linear_to_srgb(aura_result.rgb);
cogl_color_out = aura_result * cogl_color_in;`;
        this.add_glsl_snippet(Cogl.SnippetHook.FRAGMENT, declarations, code, true);
    }

    update(timeSeconds, frame, mouseEnabled, width, height, mouseX, mouseY) {
        this.set_uniform_float(this._time, 1, [timeSeconds]);
        this.set_uniform_float(this._frame, 1, [frame]);
        this.set_uniform_float(this._mouseEnabled, 1, [mouseEnabled ? 1 : 0]);
        this.set_uniform_float(this._resolution, 4, [width, height, 0, 0]);
        this.set_uniform_float(this._mouse, 4, [mouseX, mouseY, 0, 0]);
        this.set_uniform_float(this._srgb, 1, [this._colorSpace === 'srgb' ? 1 : 0]);
        this.queue_repaint();
    }
});

const AuraIndicator = GObject.registerClass(
class AuraIndicator extends PanelMenu.Button {
    _init(extension) {
        super._init(0.0, 'Aura');
        this._extension = extension;
        this.add_child(new St.Icon({
            icon_name: 'preferences-desktop-wallpaper-symbolic',
            style_class: 'system-status-icon',
        }));
        this._stats = new PopupMenu.PopupMenuItem('Aura is connecting…', {reactive: false});
        this.menu.addMenuItem(this._stats);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._next = this._action('Next Background', 'NextBackground');
        this._reload = this._action('Reload Settings', 'ReloadSettings');
        this._settings = this._action('Settings', 'OpenSettings');
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._exit = this._action('Exit', 'Exit');
    }

    _action(label, method) {
        const item = new PopupMenu.PopupMenuItem(label);
        item.connect('activate', () => this._extension.call(method));
        this.menu.addMenuItem(item);
        return item;
    }

    update(snapshot) {
        const stats = snapshot.statistics;
        const mode = snapshot.mode === 'shader'
            ? `Shader: ${snapshot.shader?.name ?? 'unknown'}`
            : snapshot.mode === 'image' ? 'Mode: Images' : 'Mode: Inactive';
        this._stats.label.text = [
            mode,
            `Image timer: ${stats.imageTimer}`,
            `Remote refresh: ${stats.remoteUpdateTimer}`,
            `Images: ${stats.imageCount} · Shown: ${stats.shown} · Skipped: ${stats.skipped}`,
            `Running: ${stats.runningDuration}`,
        ].join('\n');
        this._next.setSensitive(snapshot.mode === 'image');
    }
});

export default class AuraExtension extends Extension {
    enable() {
        this._proxy = null;
        this._imageActor = null;
        this._shaderActor = null;
        this._shaderEffect = null;
        this._rendererGeneration = 0;
        this._frameSource = 0;
        this._frame = 0;
        this._snapshot = null;
        this._indicator = null;
        this._ownerId = Gio.bus_own_name(
            Gio.BusType.SESSION,
            COMPANION_BUS,
            Gio.BusNameOwnerFlags.NONE,
            null,
            null,
            null
        );
        this._watchId = Gio.bus_watch_name(
            Gio.BusType.SESSION,
            AURA_BUS,
            Gio.BusNameWatcherFlags.NONE,
            () => this._connectProxy(),
            () => this._serviceVanished()
        );
        this._monitorsChangedId = Main.layoutManager.connect(
            'monitors-changed',
            () => this._applySnapshot(this._snapshot, true)
        );
    }

    disable() {
        if (this._watchId) Gio.bus_unwatch_name(this._watchId);
        if (this._ownerId) Gio.bus_unown_name(this._ownerId);
        if (this._monitorsChangedId)
            Main.layoutManager.disconnect(this._monitorsChangedId);
        this._watchId = 0;
        this._ownerId = 0;
        this._monitorsChangedId = 0;
        this._disconnectProxy();
        this._clearActors();
        this._destroyIndicator();
    }

    call(method) {
        if (!this._proxy) return;
        const remote = this._proxy[`${method}Remote`];
        if (remote) remote.call(this._proxy, () => {});
    }

    _connectProxy() {
        this._disconnectProxy();
        this._proxy = new AuraProxy(
            Gio.DBus.session,
            AURA_BUS,
            AURA_PATH,
            (proxy, error) => {
                if (error) {
                    console.error(`Aura: failed to connect: ${error.message}`);
                    return;
                }
                this._snapshotSignalId = proxy.connectSignal(
                    'SnapshotChanged',
                    (_proxy, _sender, [json]) => this._consumeSnapshot(json)
                );
                proxy.GetSnapshotRemote((result, callError) => {
                    if (callError) console.error(`Aura: GetSnapshot failed: ${callError.message}`);
                    else this._consumeSnapshot(result[0]);
                });
            }
        );
    }

    _disconnectProxy() {
        if (this._proxy && this._snapshotSignalId)
            this._proxy.disconnectSignal(this._snapshotSignalId);
        this._snapshotSignalId = 0;
        this._proxy = null;
    }

    _serviceVanished() {
        this._disconnectProxy();
        this._snapshot = null;
        this._clearActors();
        this._destroyIndicator();
    }

    _consumeSnapshot(json) {
        let snapshot;
        try {
            snapshot = JSON.parse(json);
            if (snapshot.version !== 1) throw new Error(`unsupported snapshot version ${snapshot.version}`);
        } catch (error) {
            console.error(`Aura: invalid snapshot: ${error.message}`);
            return;
        }
        this._snapshot = snapshot;
        this._applySnapshot(snapshot);
    }

    _applySnapshot(snapshot, forceRendererRebuild = false) {
        if (!snapshot) return;
        try {
            this._applyRendererSnapshot(snapshot, forceRendererRebuild);
        } catch (error) {
            const detail = error.message ?? String(error);
            console.error(`Aura: failed to apply ${snapshot.mode} renderer: ${detail}`);
            if (snapshot.mode === 'shader') {
                this._report(snapshot.rendererGeneration, 'error', detail);
                this._destroyShader();
            }
        }
        try {
            this._syncIndicator(snapshot);
        } catch (error) {
            const detail = error.message ?? String(error);
            console.error(`Aura: failed to update panel indicator: ${detail}`);
            this._destroyIndicator();
        }
    }

    _applyRendererSnapshot(snapshot, forceRendererRebuild) {
        if (snapshot.mode === 'inactive') {
            this._clearActors();
            return;
        }
        this._ensureImage(snapshot.imageUri);
        if (snapshot.mode === 'shader' && snapshot.shader)
            this._ensureShader(snapshot, forceRendererRebuild);
        else
            this._destroyShader();
    }

    _syncIndicator(snapshot) {
        if (!snapshot.trayEnabled) {
            this._destroyIndicator();
            return;
        }
        if (!this._indicator) {
            this._indicator = new AuraIndicator(this);
            Main.panel.addToStatusArea('aura', this._indicator);
        }
        this._indicator.update(snapshot);
    }

    _destroyIndicator() {
        this._indicator?.destroy();
        this._indicator = null;
    }

    _ensureImage(uri) {
        if (!uri) return;
        if (!this._imageActor) {
            this._imageActor = new St.Widget({reactive: false});
            Main.layoutManager._backgroundGroup.add_child(this._imageActor);
        }
        this._imageActor.set_position(0, 0);
        this._imageActor.set_size(global.stage.width, global.stage.height);
        const escaped = uri.replaceAll('"', '%22');
        this._imageActor.set_style(`background-image: url("${escaped}"); background-size: cover;`);
    }

    _ensureShader(snapshot, forceRendererRebuild) {
        const shader = snapshot.shader;
        if (!forceRendererRebuild && this._shaderActor &&
            this._rendererGeneration === snapshot.rendererGeneration)
            return;
        this._destroyShader();
        const monitor = shader.scope === 'primary'
            ? Main.layoutManager.primaryMonitor
            : {x: 0, y: 0, width: global.stage.width, height: global.stage.height};
        this._shaderActor = new St.Widget({reactive: false});
        this._shaderActor.set_position(monitor.x, monitor.y);
        this._shaderActor.set_size(monitor.width, monitor.height);
        Main.layoutManager._backgroundGroup.add_child(this._shaderActor);
        this._shaderEffect = new AuraShaderEffect(shader.gnomeGlsl, shader.colorSpace);
        this._shaderActor.add_effect_with_name('aura-shader', this._shaderEffect);
        this._rendererGeneration = snapshot.rendererGeneration;
        const internalWidth = Math.max(1, Math.round(monitor.width * shader.resolutionPercentage / 100));
        const internalHeight = Math.max(1, Math.round(monitor.height * shader.resolutionPercentage / 100));
        const intervalMs = Math.max(1, Math.round(1000 / Math.max(1, shader.targetFps)));
        this._frame = 0;
        this._frameSource = GLib.timeout_add(GLib.PRIORITY_DEFAULT, intervalMs, () => {
            if (!this._shaderEffect) return GLib.SOURCE_REMOVE;
            const [mouseX, mouseY] = global.get_pointer();
            const scaleX = internalWidth / Math.max(1, monitor.width);
            const scaleY = internalHeight / Math.max(1, monitor.height);
            this._shaderEffect.update(
                Math.max(0, Date.now() - shader.phaseStartUnixMs) / 1000,
                this._frame++,
                shader.mouseEnabled,
                internalWidth,
                internalHeight,
                shader.mouseEnabled ? (mouseX - monitor.x) * scaleX : 0,
                shader.mouseEnabled ? (mouseY - monitor.y) * scaleY : 0
            );
            return GLib.SOURCE_CONTINUE;
        });
        this._report(snapshot.rendererGeneration, 'ready', '');
    }

    _report(generation, status, detail) {
        this._proxy?.ReportRendererStatusRemote(generation, status, detail, () => {});
    }

    _destroyShader() {
        if (this._frameSource) GLib.source_remove(this._frameSource);
        this._frameSource = 0;
        this._shaderActor?.destroy();
        this._shaderActor = null;
        this._shaderEffect = null;
        this._rendererGeneration = 0;
    }

    _clearActors() {
        this._destroyShader();
        this._imageActor?.destroy();
        this._imageActor = null;
    }
}
