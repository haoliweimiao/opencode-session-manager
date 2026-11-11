#!/usr/bin/env pwsh
# 编译并运行 opencode-session-manager（release 构建）
# 用法：scripts/build-run.ps1 [传给程序的参数...]
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & "$root\target\release\opencode-session-manager.exe" @args
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
