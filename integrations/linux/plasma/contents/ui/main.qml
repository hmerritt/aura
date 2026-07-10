import QtQuick
import QtQuick.Window

Item {
    id: root

    property var snapshot: ({version: 1, mode: "inactive", rendererGeneration: 0})
    property var shaderState: snapshot.shader || ({})
    property point mousePosition: Qt.point(width / 2, height / 2)
    property int frameIndex: 0
    property bool leaseValid: snapshot.mode !== "shader"
        || Number(snapshot.leaseExpiresAtUnixMs || 0) > Date.now()
    property bool primaryScreen: !Qt.application.primaryScreen
        || root.Screen.name === Qt.application.primaryScreen.name

    function screensBounds() {
        var screens = Qt.application.screens || []
        if (screens.length === 0)
            return {x: 0, y: 0, width: width, height: height}
        var minX = screens[0].virtualX
        var minY = screens[0].virtualY
        var maxX = screens[0].virtualX + screens[0].width
        var maxY = screens[0].virtualY + screens[0].height
        for (var i = 1; i < screens.length; ++i) {
            minX = Math.min(minX, screens[i].virtualX)
            minY = Math.min(minY, screens[i].virtualY)
            maxX = Math.max(maxX, screens[i].virtualX + screens[i].width)
            maxY = Math.max(maxY, screens[i].virtualY + screens[i].height)
        }
        return {x: minX, y: minY, width: maxX - minX, height: maxY - minY}
    }

    function parseSnapshot() {
        try {
            var parsed = JSON.parse(wallpaper.configuration.Snapshot || "{}")
            if (parsed.version !== 1)
                throw new Error("unsupported snapshot version " + parsed.version)
            var generationChanged = parsed.rendererGeneration !== snapshot.rendererGeneration
            snapshot = parsed
            leaseValid = snapshot.mode !== "shader"
                || Number(snapshot.leaseExpiresAtUnixMs || 0) > Date.now()
            if (generationChanged)
                frameIndex = 0
            updateRendererStatus()
        } catch (error) {
            setRendererStatus("error", "invalid Aura snapshot: " + error)
        }
    }

    function setRendererStatus(status, detail) {
        var generation = String(snapshot.rendererGeneration || 0)
        var message = detail || ""
        if (wallpaper.configuration.AckGeneration !== generation)
            wallpaper.configuration.AckGeneration = generation
        if (wallpaper.configuration.RendererStatus !== status)
            wallpaper.configuration.RendererStatus = status
        if (wallpaper.configuration.RendererDetail !== message)
            wallpaper.configuration.RendererDetail = message
    }

    function updateRendererStatus() {
        if (snapshot.mode !== "shader") {
            setRendererStatus("ready", "")
            return
        }
        if (!leaseValid) {
            setRendererStatus("error", "Aura renderer lease expired; showing the last image")
            return
        }
        if (!shaderState.plasmaVertexUri || !shaderState.plasmaFragmentUri) {
            setRendererStatus("error", "Aura snapshot does not contain Plasma shader assets")
            return
        }
        if (shaderEffect.status === ShaderEffect.Error)
            setRendererStatus("error", shaderEffect.log)
        else if (shaderEffect.status === ShaderEffect.Compiled)
            setRendererStatus("ready", "")
        else
            setRendererStatus("waiting", shaderEffect.log || "compiling Qt shaders")
    }

    Component.onCompleted: parseSnapshot()

    Connections {
        target: wallpaper.configuration
        function onSnapshotChanged() { root.parseSnapshot() }
    }

    Image {
        id: imageLayer
        anchors.fill: parent
        source: root.snapshot.imageUri || ""
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        visible: source !== ""
    }

    ShaderEffect {
        id: shaderEffect
        anchors.fill: parent
        visible: root.snapshot.mode === "shader"
            && root.leaseValid
            && (root.shaderState.scope !== "primary" || root.primaryScreen)

        property real aura_time_seconds: {
            var now = Date.now() + root.frameIndex * 0
            return Math.max(0, now - Number(root.shaderState.phaseStartUnixMs || now)) / 1000
        }
        property real aura_frame_index: root.frameIndex
        property real aura_mouse_enabled: root.shaderState.mouseEnabled ? 1 : 0
        property real aura_padding: 0
        property real aura_color_space_srgb: root.shaderState.colorSpace === "srgb" ? 1 : 0
        property vector4d aura_resolution: {
            var bounds = root.shaderState.scope === "virtual" ? root.screensBounds() : {width: root.width, height: root.height}
            var scale = Math.max(1, Number(root.shaderState.resolutionPercentage || 100)) / 100
            return Qt.vector4d(Math.max(1, bounds.width * scale), Math.max(1, bounds.height * scale), 0, 0)
        }
        property vector4d aura_origin: {
            var scale = Math.max(1, Number(root.shaderState.resolutionPercentage || 100)) / 100
            if (root.shaderState.scope !== "virtual")
                return Qt.vector4d(0, 0, root.width * scale, root.height * scale)
            var bounds = root.screensBounds()
            return Qt.vector4d(
                (root.Screen.virtualX - bounds.x) * scale,
                (root.Screen.virtualY - bounds.y) * scale,
                root.width * scale,
                root.height * scale
            )
        }
        property vector4d aura_mouse: root.shaderState.mouseEnabled ? Qt.vector4d(
            aura_origin.x + root.mousePosition.x * aura_origin.z / Math.max(1, root.width),
            aura_origin.y + root.mousePosition.y * aura_origin.w / Math.max(1, root.height),
            0,
            0
        ) : Qt.vector4d(0, 0, 0, 0)

        vertexShader: root.shaderState.plasmaVertexUri || ""
        fragmentShader: root.shaderState.plasmaFragmentUri || ""
        onStatusChanged: root.updateRendererStatus()
    }

    HoverHandler {
        enabled: root.shaderState.mouseEnabled === true
        onPointChanged: root.mousePosition = point.position
    }

    Timer {
        interval: Math.max(1, Math.round(1000 / Math.max(1, Number(root.shaderState.targetFps || 60))))
        repeat: true
        running: shaderEffect.visible
        onTriggered: root.frameIndex += 1
    }

    Timer {
        interval: 1000
        repeat: true
        running: root.snapshot.mode === "shader"
        onTriggered: {
            root.leaseValid = Number(root.snapshot.leaseExpiresAtUnixMs || 0) > Date.now()
            root.updateRendererStatus()
        }
    }
}
