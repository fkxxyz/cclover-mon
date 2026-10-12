param(
    [Parameter(Mandatory = $true)]
    [string]$ExecutablePath,

    [Parameter(Mandatory = $true)]
    [version]$MinimumPawnIoVersion
)

$ErrorActionPreference = "Stop"
$serviceName = "cclover-mon-server"
$serviceCreated = $false
$serviceDir = $null

function Wait-ServiceState([string]$Name, [string]$Expected, [int]$TimeoutSeconds) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        $service = Get-Service -Name $Name -ErrorAction SilentlyContinue
        if ($service -and $service.Status.ToString() -eq $Expected) { return }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $deadline)
    $actual = (Get-Service -Name $Name -ErrorAction SilentlyContinue).Status
    throw "Service state timeout: expected $Expected, got $actual"
}

try {
    if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
        throw "Refusing to touch existing service '$serviceName'"
    }
    if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
        throw "Server executable not found: $ExecutablePath"
    }

    $driver = Get-CimInstance Win32_SystemDriver | Where-Object Name -eq "PawnIO"
    if (-not $driver -or $driver.State -ne "Running") {
        throw "PawnIO must already be installed and running; refusing provisioning during SCM validation"
    }
    $driverPath = $driver.PathName -replace '^\\\?\\', ''
    $driverVersion = [version](Get-Item -LiteralPath $driverPath).VersionInfo.ProductVersion
    if ($driverVersion -lt $MinimumPawnIoVersion) {
        throw "PawnIO $driverVersion is below required $MinimumPawnIoVersion; refusing driver update"
    }

    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()

    $serviceDir = Join-Path $env:ProgramData ("cclover-mon-scm-smoke-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $serviceDir | Out-Null
    $serviceExe = Join-Path $serviceDir "cclover-mon-server.exe"
    Copy-Item -LiteralPath $ExecutablePath -Destination $serviceExe

    $binaryPath = '"' + $serviceExe + '" --service --bind 127.0.0.1:' + $port
    & sc.exe create $serviceName "binPath=" $binaryPath "start=" "demand" "obj=" "LocalSystem" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "SCM service creation failed: $LASTEXITCODE" }
    $serviceCreated = $true

    & sc.exe start $serviceName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "SCM service start request failed: $LASTEXITCODE" }
    Wait-ServiceState $serviceName "Running" 30

    $ready = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ([DateTime]::UtcNow -lt $deadline) {
        try {
            $response = Invoke-WebRequest -Uri "http://127.0.0.1:$port/readyz" -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) { $ready = $true; break }
        } catch {
            Start-Sleep -Milliseconds 300
        }
    }
    if (-not $ready) { throw "Service did not make /readyz ready" }

    & sc.exe stop $serviceName | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "SCM service stop request failed: $LASTEXITCODE" }
    Wait-ServiceState $serviceName "Stopped" 30
    $service = Get-CimInstance Win32_Service -Filter "Name='$serviceName'"
    if ($service.ProcessId -ne 0) { throw "Service process remains after STOP: PID $($service.ProcessId)" }

    Write-Output "PASS: SCM RUNNING, /readyz HTTP 200, cooperative STOPPED, process exited"
} finally {
    if ($serviceCreated) {
        $service = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
        if ($service -and $service.Status.ToString() -ne "Stopped") {
            & sc.exe stop $serviceName | Out-Null
            try {
                Wait-ServiceState $serviceName "Stopped" 30
            } catch {
                $processId = (Get-CimInstance Win32_Service -Filter "Name='$serviceName'").ProcessId
                if ($processId -gt 0) { Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue }
                Wait-ServiceState $serviceName "Stopped" 10
            }
        }

        & sc.exe delete $serviceName | Out-Null
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $service = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
            if (-not $service) { break }
            Start-Sleep -Milliseconds 250
        } while ([DateTime]::UtcNow -lt $deadline)
        if (Get-Service -Name $serviceName -ErrorAction SilentlyContinue) {
            throw "SCM service cleanup failed for '$serviceName'"
        }
        if ($serviceDir) { Remove-Item -LiteralPath $serviceDir -Recurse -Force }
        Write-Output "Cleanup: temporary SCM service and executable removed"
    }
}
