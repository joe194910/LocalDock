[CmdletBinding()]
param(
    [ValidateSet('x64', 'arm64')]
    [string]$Architecture = 'x64'
)

$ErrorActionPreference = 'Stop'

if (-not (Get-Command npm.cmd -ErrorAction SilentlyContinue)) {
    throw '未找到 npm.cmd，请先安装受支持的 Node.js LTS。'
}
if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path (Join-Path $cargoBin 'cargo.exe')) {
        $env:Path = "$cargoBin;$env:Path"
    }
}
if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
    throw '未找到 Cargo，请先安装 Rust stable-msvc 工具链。'
}

$target = if ($Architecture -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }

Write-Host "正在检查前端，目标架构：$target"
npm.cmd run build:web
if ($LASTEXITCODE -ne 0) {
    throw '前端构建失败。'
}

Write-Host '正在生成 Windows NSIS 安装包'
npm.cmd run tauri:build -- --target $target
if ($LASTEXITCODE -ne 0) {
    throw 'Tauri Windows 安装包构建失败。'
}
