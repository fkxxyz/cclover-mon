param(
    [Parameter(Mandatory = $true)]
    [string]$Repository,

    [string]$Profile,

    [ValidateSet("ordinary", "elevated")]
    [string]$Privilege = "ordinary",

    [switch]$Doctor
)

$ErrorActionPreference = "Stop"
$TaskName = "cclover-mon-validation-elevated"
$BridgeRoot = Join-Path $env:LOCALAPPDATA "cclover-mon-validation"
$Runner = Join-Path $Repository "tools\windows-validation\runner.ps1"
$RequestPath = Join-Path $BridgeRoot "request.json"
$ResponsePath = Join-Path $BridgeRoot "response.json"
$OutputPath = Join-Path $BridgeRoot "output.log"

if ($Doctor) {
    $failed = $false

    if (Get-Command bun -ErrorAction SilentlyContinue) {
        Write-Host "Windows Bun: available"
    }
    else {
        Write-Host "Windows Bun: missing"
        $failed = $true
    }

    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "Windows Cargo: available"
    }
    else {
        Write-Host "Windows Cargo: missing"
        $failed = $true
    }

    if (Test-Path -LiteralPath $Runner) {
        Write-Host "Repository runner: available"
    }
    else {
        Write-Host "Repository runner: missing ($Runner)"
        $failed = $true
    }

    if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
        Write-Host "Elevated bridge: provisioned"
    }
    else {
        Write-Host "Elevated bridge: not provisioned"
        $failed = $true
    }

    if ($failed) { exit 1 }
    exit 0
}

if (-not $Profile) {
    [Console]::Error.WriteLine("Profile is required")
    exit 2
}

if (-not (Test-Path -LiteralPath $Runner)) {
    [Console]::Error.WriteLine("Repository runner not found: $Runner")
    exit 2
}

if ($Privilege -eq "ordinary") {
    & powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $Runner -Repository $Repository -Profile $Profile
    exit $LASTEXITCODE
}

if (-not (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine("Elevated validation bridge is not provisioned. Run: bun windows-validate.ts install")
    exit 2
}

New-Item -ItemType Directory -Force -Path $BridgeRoot | Out-Null
$mutex = New-Object System.Threading.Mutex($false, "Local\cclover-mon-validation")
$locked = $false
try {
    $locked = $mutex.WaitOne(0)
    if (-not $locked) {
        [Console]::Error.WriteLine("Another elevated Windows validation is already running")
        exit 2
    }

    $task = Get-ScheduledTask -TaskName $TaskName
    if ($task.State -eq "Running") {
        [Console]::Error.WriteLine("Elevated validation task is already running")
        exit 2
    }

    $nonce = [Guid]::NewGuid().ToString("N")
    Remove-Item -Force -ErrorAction SilentlyContinue $ResponsePath, $OutputPath
    @{
        nonce = $nonce
        repository = $Repository
        profile = $Profile
    } | ConvertTo-Json -Compress | Set-Content -LiteralPath $RequestPath -Encoding UTF8

    Start-ScheduledTask -TaskName $TaskName
    Write-Host "Running elevated Windows validation profile '$Profile'..."

    $deadline = [DateTime]::UtcNow.AddHours(2)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $ResponsePath) {
            try {
                $response = Get-Content -LiteralPath $ResponsePath -Raw | ConvertFrom-Json
                if ($response.nonce -eq $nonce) {
                    if (Test-Path -LiteralPath $OutputPath) {
                        Get-Content -LiteralPath $OutputPath
                    }
                    if ($response.message) {
                        [Console]::Error.WriteLine([string]$response.message)
                    }
                    exit [int]$response.exitCode
                }
            }
            catch {
                # The worker may still be replacing the response file.
            }
        }
        Start-Sleep -Milliseconds 250
    }

    [Console]::Error.WriteLine("Timed out waiting for elevated validation task")
    exit 124
}
finally {
    if ($locked) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
