param([Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version)

$ErrorActionPreference = "Stop"

$identifier = "ManusolJ.Loregalia"
$repository = "https://github.com/ManusolJ/loregalia"
$schema = "1.28.0"
$installer = "loregalia_${Version}_x64-setup.exe"
$root = Split-Path -Parent $PSScriptRoot
$output = Join-Path $root "target\winget\$Version"

$release = Invoke-RestMethod "https://api.github.com/repos/ManusolJ/loregalia/releases/tags/v$Version"
$asset = $release.assets | Where-Object { $_.name -eq $installer }
if (-not $asset) { throw "Release v$Version has no $installer." }
if ($asset.digest -notmatch '^sha256:([0-9a-f]{64})$') { throw "GitHub lists no checksum for $installer." }
$hash = $Matches[1].ToUpper()
$date = ([datetime]$release.published_at).ToUniversalTime().ToString("yyyy-MM-dd")

$files = @{
    "$identifier.yaml" = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.version.$schema.schema.json

PackageIdentifier: $identifier
PackageVersion: $Version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $schema
"@
    "$identifier.installer.yaml" = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.installer.$schema.schema.json

PackageIdentifier: $identifier
PackageVersion: $Version
InstallerType: nullsoft
Scope: user
InstallModes:
- interactive
- silent
InstallerSwitches:
  Upgrade: /NS
UpgradeBehavior: install
ReleaseDate: $date
AppsAndFeaturesEntries:
- DisplayName: Loregalia
  Publisher: Manu
  ProductCode: Loregalia
Installers:
- Architecture: x64
  InstallerUrl: $repository/releases/download/v$Version/$installer
  InstallerSha256: $hash
ManifestType: installer
ManifestVersion: $schema
"@
    "$identifier.locale.en-US.yaml" = @"
# yaml-language-server: `$schema=https://aka.ms/winget-manifest.defaultLocale.$schema.schema.json

PackageIdentifier: $identifier
PackageVersion: $Version
PackageLocale: en-US
Publisher: ManusolJ
PublisherUrl: https://github.com/ManusolJ
PublisherSupportUrl: $repository/issues
PackageName: Loregalia
PackageUrl: $repository
License: MIT
LicenseUrl: $repository/blob/main/LICENSE
ShortDescription: Plan what to watch, read and play next
Description: |-
  A local-first planner for movies, shows, books and games. Keep lists, statuses and tags, see what is
  coming soon, and let it suggest what to pick up next. The library stays on your computer, with no
  account.
Moniker: loregalia
Tags:
- books
- games
- library
- local-first
- movies
- planner
- tv-shows
- watchlist
ReleaseNotesUrl: $repository/releases/tag/v$Version
ManifestType: defaultLocale
ManifestVersion: $schema
"@
}

New-Item -ItemType Directory -Force $output | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
foreach ($file in $files.Keys) {
    $text = $files[$file].Replace("`r`n", "`n") + "`n"
    [System.IO.File]::WriteAllText((Join-Path $output $file), $text, $utf8)
}

winget validate --manifest $output --disable-interactivity
if ($LASTEXITCODE -ne 0) { throw "winget did not accept the manifest." }

"Manifest: $output"
