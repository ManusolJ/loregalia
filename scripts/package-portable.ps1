$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$versionLine = Select-String -Path "Cargo.toml" -Pattern '^version = "(.+)"' | Select-Object -First 1
if (-not $versionLine) { throw "Could not read the workspace version from Cargo.toml." }
$version = $versionLine.Matches[0].Groups[1].Value

cargo build --release --locked -p loregalia
if ($LASTEXITCODE -ne 0) { throw "The release build failed." }

$output = Join-Path $root "target\installer"
$staging = Join-Path $output "portable"
$archive = Join-Path $output "loregalia_${version}_x64-portable.zip"

if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Force $staging | Out-Null
Copy-Item "target\release\loregalia.exe" $staging
Copy-Item "LICENSE" $staging
Set-Content -Path (Join-Path $staging "portable") -Value "Loregalia keeps its data in the data folder next to this file. Delete this file to use the installed data folder instead." -Encoding ascii

if (Test-Path $archive) { Remove-Item $archive -Force }
Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $archive
Remove-Item $staging -Recurse -Force

"Portable build: $archive"
