param(
    [Parameter(Mandatory = $true)]
    [string]$Name
)

$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $root

$readOnly = @(
    'docs', 'scripts', 'xtask', '.githooks', '.github', '.cargo',
    'AGENTS.md', 'README.md', 'LICENSE', 'opencode.json', 'clippy.toml', 'rust-toolchain.toml'
)

$linuxRoot = '/' + $root.Substring(0, 1).ToLower() + ($root.Substring(2) -replace '\\', '/')
$mounts = @('.')

foreach ($item in $readOnly) {
    if (Test-Path -LiteralPath (Join-Path $root $item)) {
        $mounts += "$(Join-Path $root $item):ro"
    }
}

$secrets = @(Get-ChildItem -LiteralPath $root -Recurse -Force -File |
    Where-Object { $_.FullName -notmatch '\\(target|\.git)\\' } |
    Where-Object { $_.Name -match '^\.env|\.env$|\.(pem|key)$' -and $_.Name -notmatch '\.env\.example$' })

$blankFolder = Join-Path $HOME '.loregalia-sandbox'
New-Item -ItemType Directory -Force -Path $blankFolder | Out-Null

$index = 0
foreach ($secret in $secrets) {
    $blank = Join-Path $blankFolder "blank-$index"
    if (-not (Test-Path -LiteralPath $blank)) {
        New-Item -ItemType File -Path $blank | Out-Null
    }

    $relative = $secret.FullName.Substring($root.Length + 1) -replace '\\', '/'
    $mounts += "${blank}:$linuxRoot/${relative}:ro"
    Write-Output "Hiding from the sandbox: $relative"
    $index++
}

sbx create --name $Name opencode @mounts
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

sbx exec $Name sh scripts/sandbox-setup.sh
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Output "Ready. Start the agent with: sbx run --name $Name -- --auto"
Write-Output "Create a new sandbox with this script after adding a secret file."
