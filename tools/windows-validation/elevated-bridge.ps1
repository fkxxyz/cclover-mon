$ErrorActionPreference = "Stop"
$BridgeRoot = Join-Path $env:LOCALAPPDATA "cclover-mon-validation"
$RequestPath = Join-Path $BridgeRoot "request.json"
$ResponsePath = Join-Path $BridgeRoot "response.json"
$OutputPath = Join-Path $BridgeRoot "output.log"

function Write-Response([string]$Nonce, [int]$ExitCode, [string]$Message = "") {
    $temp = "$ResponsePath.tmp"
    @{
        nonce = $Nonce
        exitCode = $ExitCode
        message = $Message
    } | ConvertTo-Json -Compress | Set-Content -LiteralPath $temp -Encoding UTF8
    Move-Item -Force -LiteralPath $temp -Destination $ResponsePath
}

if (-not (Test-Path -LiteralPath $RequestPath)) {
    exit 2
}

$nonce = ""
try {
    $request = Get-Content -LiteralPath $RequestPath -Raw | ConvertFrom-Json
    $nonce = [string]$request.nonce
    $repository = [string]$request.repository
    $profile = [string]$request.profile

    if (-not $nonce -or -not $repository -or -not $profile) {
        throw "Invalid validation request"
    }

    $runner = Join-Path $repository "tools\windows-validation\runner.ps1"
    if (-not (Test-Path -LiteralPath $runner)) {
        throw "Repository runner not found: $runner"
    }

    & powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $runner -Repository $repository -Profile $profile *> $OutputPath
    Write-Response -Nonce $nonce -ExitCode $LASTEXITCODE
}
catch {
    Write-Response -Nonce $nonce -ExitCode 1 -Message $_.Exception.Message
}
