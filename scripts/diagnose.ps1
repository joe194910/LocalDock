[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

Write-Host '=== LocalDock Windows 诊断 ==='
Write-Host "系统: $([System.Environment]::OSVersion.VersionString)"
Write-Host "架构: $env:PROCESSOR_ARCHITECTURE"
Write-Host "当前用户: $env:USERNAME"

$commands = @(
    @{ Name = 'Node.js'; Command = 'node.exe'; Arguments = @('--version') },
    @{ Name = 'npm'; Command = 'npm.cmd'; Arguments = @('--version') },
    @{ Name = 'Rust'; Command = 'rustc.exe'; Arguments = @('--version') },
    @{ Name = 'Cargo'; Command = 'cargo.exe'; Arguments = @('--version') },
    @{ Name = 'Git'; Command = 'git.exe'; Arguments = @('--version') },
    @{ Name = 'Java'; Command = 'java.exe'; Arguments = @('-version') },
    @{ Name = 'Maven'; Command = 'mvn.cmd'; Arguments = @('--version') },
    @{ Name = 'WSL'; Command = 'wsl.exe'; Arguments = @('--status') }
)

foreach ($item in $commands) {
    $resolved = Get-Command $item.Command -ErrorAction SilentlyContinue
    if (-not $resolved) {
        Write-Host "[缺失] $($item.Name)"
        continue
    }
    Write-Host "[已发现] $($item.Name): $($resolved.Source)"
    & $item.Command @($item.Arguments) 2>&1 | Select-Object -First 3
}

$assetPath = Join-Path $env:LOCALAPPDATA 'LocalDock\assets.json'
Write-Host "资产文件: $assetPath"
if (Test-Path -LiteralPath $assetPath) {
    Write-Host '[已发现] 本地资产文件存在'
} else {
    Write-Host '[信息] 尚未创建本地资产文件'
}
