param(
    [Parameter(Mandatory = $true)]
    [string]$BinaryPath,
    [string]$Bind = "127.0.0.1:9847"
)

$ErrorActionPreference = "Stop"
$serviceName = "cclover-mon-server"
$resolvedBinary = (Resolve-Path $BinaryPath).Path
$commandLine = '"{0}" --service --bind {1}' -f $resolvedBinary, $Bind

if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
    throw "Service '$serviceName' already exists."
}

New-Service `
    -Name $serviceName `
    -DisplayName "cclover-mon server" `
    -Description "Headless cclover-mon system monitor" `
    -BinaryPathName $commandLine `
    -StartupType Automatic | Out-Null

Write-Host "Installed $serviceName. Start it with: Start-Service $serviceName"
