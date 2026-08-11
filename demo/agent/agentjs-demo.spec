# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path

project = Path(SPECPATH).parents[1]
datas = [(str(project / "frontend" / "agent-chat.html"), "frontend")]
test262_report = project / "reports" / "full-test262-summary.json"
if test262_report.is_file():
    datas.append((str(test262_report), "reports"))

a = Analysis(
    [str(project / "demo" / "agent" / "server.py")],
    pathex=[str(project)],
    binaries=[
        (str(project / "target" / "release" / "agentjs.exe"), "."),
        (str(project / "boa" / "target" / "release" / "boa.exe"), "."),
        (str(project / "target" / "oxide-compare" / "release" / "oxide.exe"), "."),
    ],
    datas=datas,
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
