param(
    [Parameter(Mandatory = $true)]
    [string]$Repository,

    [Parameter(Mandatory = $true)]
    [string]$Profile
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath (Join-Path $Repository "validate.ts"))) {
    [Console]::Error.WriteLine("validate.ts not found under repository: $Repository")
    exit 2
}

Push-Location -LiteralPath $Repository
try {
    & bun validate.ts $Profile
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
