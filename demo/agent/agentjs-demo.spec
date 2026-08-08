# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path

project = Path(SPECPATH).parents[1]

a = Analysis(
    [str(project / "demo" / "agent" / "server.py")],
    pathex=[str(project)],
    binaries=[(str(project / "target" / "release" / "agentjs.exe"), ".")],
    datas=[(str(project / "frontend" / "agent-chat.html"), "frontend")],
    hiddenimports=["webview"],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="AgentJS-Demo",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
