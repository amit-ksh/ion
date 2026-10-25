$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath 'D:/personal/apps/waku'
$workspace = (Get-Location).Path
if ((Get-Content -Raw docs/qa/2026-10-18-ion/installer-build.exit).Trim() -ne '0') { throw 'Installer build has not passed' }
$snapshot = Get-Content -Raw .waku-cache/ion-qa-0.1.23-source.json | ConvertFrom-Json
foreach ($entry in $snapshot.files) {
  if ((Get-FileHash -LiteralPath $entry.path -Algorithm SHA256).Hash -ne $entry.sha256) { throw ('Source snapshot mismatch: ' + $entry.path) }
}
$release = Join-Path $workspace 'target/release'
$packageName = 'ion-0.1.23-x86_64-pc-windows-msvc'
$archive = Join-Path $release ($packageName + '.zip')
$installer = Get-Item -LiteralPath (Join-Path $release 'ion-0.1.23-x86_64-Setup.exe')
if ($installer.VersionInfo.FileVersion.Trim() -ne '0.1.23') { throw 'Wrong installer version' }
$entries = @(& tar -tf $archive)
if ($LASTEXITCODE -ne 0 -or !$entries.Count) { throw 'Cannot list package' }
foreach ($entry in $entries) {
  if (!$entry.StartsWith($packageName + '/') -or $entry.Contains('../') -or $entry.Contains('..\')) { throw ('Unsafe archive path: ' + $entry) }
}
$cache = (Resolve-Path -LiteralPath '.waku-cache').Path
if (!$cache.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or ((Get-Item -LiteralPath $cache).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Unsafe verification cache' }
$verify = Join-Path $cache ('ion-qa-0.1.23-verify-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($verify) | Out-Null
& tar -xf $archive -C $verify
if ($LASTEXITCODE -ne 0) { throw 'Extraction failed' }
$package = Join-Path $verify $packageName
$required = @('waku.exe','waku-daemon.exe','waku_js_repl.exe','waku_computer_use.exe','cua_driver_sdk.dll','cua-driver-uia.exe','resources/computer-use/pi-extension.ts','resources/computer-use/CUA-LICENSE','resources/skills/waku-computer-use/SKILL.md','nemo-speech/bin/nemo-speech.exe','nemo-speech/share/nemo-speech/model-index.json','nemo-speech/share/licenses/nemo-speech/LICENSE','LICENSE')
foreach ($file in $required) { if ((Get-Item -LiteralPath (Join-Path $package $file)).Length -eq 0) { throw ('Empty/missing package file: ' + $file) } }
$binaryChecks = foreach ($file in @('waku.exe','waku-daemon.exe','waku_js_repl.exe','waku_computer_use.exe')) {
  $hash = (Get-FileHash -LiteralPath (Join-Path $package $file) -Algorithm SHA256).Hash
  if ($hash -ne (Get-FileHash -LiteralPath (Join-Path $release $file) -Algorithm SHA256).Hash) { throw ('Binary mismatch: ' + $file) }
  [ordered]@{name=$file;sha256=$hash}
}
$desktopText = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes((Join-Path $package 'waku.exe')))
if (!$desktopText.Contains([IO.File]::ReadAllText((Join-Path $workspace 'assets/icons/mic.svg')).Trim())) { throw 'Embedded microphone icon missing' }
$desktopText = $null
$daemonHelp = (& (Join-Path $package 'waku-daemon.exe') --help 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $daemonHelp -notmatch 'Usage') { throw 'Packaged daemon smoke failed' }
$speechVersion = (& (Join-Path $package 'nemo-speech/bin/nemo-speech.exe') --version 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $speechVersion -notmatch '0\.1\.0') { throw 'Packaged speech smoke failed' }
$artifacts = foreach ($file in @($installer.FullName,$archive)) {
  $item = Get-Item -LiteralPath $file
  [ordered]@{name=$item.Name;bytes=$item.Length;sha256=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash}
}
$manifest = [ordered]@{app='ion';version='0.1.23';branch='ion-prod';sourceBaseCommit=$snapshot.sourceCommit;workingTreeChanges=$true;sourceFiles=$snapshot.files;verifiedAt=[DateTimeOffset]::Now.ToString('o');authenticode=(Get-AuthenticodeSignature -LiteralPath $installer.FullName).Status.ToString();artifacts=$artifacts;requiredFiles=$required.Count;binaries=$binaryChecks;daemonSmoke='--help exit 0';speechSmoke=$speechVersion;extractedPackage=$package;nativeAcceptance='See docs/qa/2026-10-18-ion/README.md'}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $release 'ion-0.1.23-build-manifest.json')
$artifacts | ForEach-Object { $_.sha256.ToLowerInvariant() + '  ' + $_.name } | Set-Content -LiteralPath (Join-Path $release 'ion-0.1.23-SHA256SUMS.txt')
[pscustomobject]$manifest | Select-Object version,authenticode,requiredFiles,daemonSmoke,speechSmoke,extractedPackage | ConvertTo-Json
