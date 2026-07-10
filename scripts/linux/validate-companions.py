#!/usr/bin/env python3
"""Validate the source-tree Linux companion contracts without installing them."""

from __future__ import annotations

import json
from pathlib import Path
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[2]
GNOME = ROOT / "integrations" / "linux" / "gnome"
PLASMA = ROOT / "integrations" / "linux" / "plasma"


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as source:
        value = json.load(source)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def require_text(path: Path, fragments: tuple[str, ...]) -> None:
    source = path.read_text(encoding="utf-8")
    for fragment in fragments:
        if fragment not in source:
            raise ValueError(f"{path} is missing required contract text {fragment!r}")


def main() -> None:
    gnome = load_json(GNOME / "metadata.json")
    if gnome.get("uuid") != "aura@hmerritt.github.io":
        raise ValueError("GNOME metadata has the wrong extension UUID")
    versions = {int(version) for version in gnome.get("shell-version", [])}
    if 45 not in versions:
        raise ValueError("GNOME metadata must include the GNOME 45 compatibility floor")

    plasma = load_json(PLASMA / "metadata.json")
    if plasma.get("KPackageStructure") != "Plasma/Wallpaper":
        raise ValueError("Plasma companion must be a wallpaper package")
    if plasma.get("KPlugin", {}).get("Id") != "io.github.hmerritt.Aura":
        raise ValueError("Plasma metadata has the wrong plugin ID")

    config_path = PLASMA / "contents" / "config" / "main.xml"
    ET.parse(config_path)
    require_text(
        config_path,
        ("Snapshot", "AckGeneration", "RendererStatus", "RendererDetail"),
    )
    require_text(
        GNOME / "extension.js",
        (
            "io.github.hmerritt.Aura1",
            "SnapshotChanged",
            "ReportRendererStatus",
            "Shell.GLSLEffect",
        ),
    )
    require_text(
        PLASMA / "contents" / "ui" / "main.qml",
        ("ShaderEffect", "leaseExpiresAtUnixMs", "AckGeneration"),
    )


if __name__ == "__main__":
    main()
