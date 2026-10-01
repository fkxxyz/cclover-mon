$ErrorActionPreference = "Stop"
$TaskName = "cclover-mon-validation-elevated"
$BridgeRoot = Join-Path $env:LOCALAPPDATA "cclover-mon-validation"
$SourceBridge = Join-Path $PSScriptRoot "elevated-bridge.ps1"
$InstalledBridge = Join-Path $BridgeRoot "elevated-bridge.ps1"

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$admin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $admin) {
    $arguments = @(
        "-NoLogo",
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", ('"{0}"' -f $PSCommandPath)
    ) -join " "
    $process = Start-Process -FilePath "powershell.exe" -ArgumentList $arguments -Verb RunAs -Wait -PassThru
    exit $process.ExitCode
}

New-Item -ItemType Directory -Force -Path $BridgeRoot | Out-Null
Copy-Item -Force -LiteralPath $SourceBridge -Destination $InstalledBridge

$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$actionArguments = '-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}"' -f $InstalledBridge
$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $actionArguments
$taskPrincipal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Hours 2)
Register-ScheduledTask -TaskName $TaskName -Action $action -Principal $taskPrincipal -Settings $settings -Force | Out-Null

Write-Host "Installed elevated Windows validation bridge: $TaskName"
