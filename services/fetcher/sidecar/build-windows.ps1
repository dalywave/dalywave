<#
Загрузчик DalyWave для приложения Windows (sidecar Tauri) → services/fetcher/dist/sidecar/ —
та же раскладка, что у build.sh для macOS (SIDECAR.md, «Windows x64»):
  dalywave-fetcher-x86_64-pc-windows-msvc.exe   Python 3.13 + yt-dlp (+ejs) + ytmusicapi + плагин bgutil (PyInstaller, один файл)
  deno-x86_64-pc-windows-msvc.exe               JS-задачи YouTube и сервис PO-токенов
  ffmpeg-….exe, ffprobe-….exe                   FFmpeg для звука и клипов, LGPL 2.1+ (ffmpeg-windows.sh, кросс-сборка на Linux)
  fpcalc-….exe                                  chromaprint, официальная сборка
  resources/bgutil-server/                      сервер PO-токенов bgutil: TypeScript + node_modules, canvas для win32-x64
  licenses/, MANIFEST.json                      лицензии; версии, источники, sha256 и размеры

  pwsh services/fetcher/sidecar/build-windows.ps1 -Ffmpeg <папка с ffmpeg.exe и ffprobe.exe> [-Clean]

Запускать на Windows x64 (PyInstaller не собирает накрест). Нужны uv и Node ≥ 22 (npm); tar есть
в Windows 10+. На машине пользователя ничего из этого не нужно. В CI — .github/workflows/desktop.yml.
#>
param(
  [Parameter(Mandatory = $true)] [string] $Ffmpeg,
  [switch] $Clean
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$PSNativeCommandUseErrorActionPreference = $true
Set-StrictMode -Version 3

$Here = $PSScriptRoot
$Fetcher = Split-Path -Parent $Here
$Dist = Join-Path $Fetcher 'dist'
$Build = Join-Path $Dist 'build'
$Out = Join-Path $Dist 'sidecar'
$Downloads = Join-Path $Build 'downloads'
$Triple = 'x86_64-pc-windows-msvc'
# Системный bsdtar: tar из Git for Windows принимает «C:» за адрес удалённой машины
$Tar = Join-Path $env:SystemRoot 'System32\tar.exe'

# ── Версии и контрольные суммы — как в build.sh и SIDECAR.md ──
$PythonVersion = '3.13.18'
$DenoVersion = '2.9.8'
$DenoSha = 'cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf'
$ChromaprintVersion = '1.6.2'
$FpcalcSha = 'b74369726676670af3ab0eadff45f960cf969fdf4a87a971ba37eaf8d530e874'
$ChromaprintSrcSha = 'd54ada62c16b2db396d3251ec54fc7a58909c163131a1cfcf7fa6407dbc14360'
$BgutilVersion = '2.0.2'
$BgutilSha = '1f2ae9ec39405b8e80397dfc2551f58deed09d8dff4b2b8fbc314df8e091c0a1'
$CanvasVersion = '3.2.3'
$CanvasSha = 'ba953cc8c38303ab94cc83461c7561506a5a3af37d6c678de4a47914d2d0bb48'

$DenoUrl = "https://github.com/denoland/deno/releases/download/v$DenoVersion/deno-$Triple.zip"
$FpcalcUrl = "https://github.com/acoustid/chromaprint/releases/download/v$ChromaprintVersion/chromaprint-fpcalc-$ChromaprintVersion-windows-x86_64.zip"
$ChromaprintSrcUrl = "https://github.com/acoustid/chromaprint/releases/download/v$ChromaprintVersion/chromaprint-$ChromaprintVersion.tar.gz"
$BgutilUrl = "https://github.com/Brainicism/bgutil-ytdlp-pot-provider/archive/refs/tags/$BgutilVersion.tar.gz"
$CanvasUrl = "https://github.com/Automattic/node-canvas/releases/download/v$CanvasVersion/canvas-v$CanvasVersion-napi-v7-win32-x64.tar.gz"

function Log([string] $Message) { Write-Host "==> $Message" }
function Fail([string] $Message) { throw "ошибка: $Message" }
function Sha256([string] $Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

# Скачать в dist/build/downloads и сверить sha256
function Fetch([string] $Url, [string] $Sha, [string] $Name) {
  $file = Join-Path $Downloads $Name
  if ((Test-Path -LiteralPath $file) -and ((Sha256 $file) -eq $Sha)) { return $file }
  Log "скачиваю $Url"
  $part = "$file.part"
  Invoke-WebRequest -Uri $Url -OutFile $part -MaximumRetryCount 3 -RetryIntervalSec 5
  $actual = Sha256 $part
  if ($actual -ne $Sha) { Fail "sha256 не совпала: $Url ($actual)" }
  Move-Item -Force -LiteralPath $part -Destination $file
  return $file
}

function Fresh([string] $Path) {
  if (Test-Path -LiteralPath $Path) { Remove-Item -Recurse -Force -LiteralPath $Path }
  New-Item -ItemType Directory -Force -Path $Path | Out-Null
}

foreach ($tool in 'uv', 'npm') {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "для сборки нужен $tool" }
}
if (-not (Test-Path -LiteralPath $Tar)) { Fail "нет $Tar" }
foreach ($exe in 'ffmpeg.exe', 'ffprobe.exe') {
  if (-not (Test-Path -LiteralPath (Join-Path $Ffmpeg $exe))) { Fail "в $Ffmpeg нет $exe (ffmpeg-windows.sh)" }
}

if ($Clean) {
  foreach ($dir in $Build, $Out) { if (Test-Path -LiteralPath $dir) { Remove-Item -Recurse -Force -LiteralPath $dir } }
}
foreach ($dir in $Downloads, (Join-Path $Out 'resources'), (Join-Path $Out 'licenses')) {
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

# ── deno ──
Log "deno $DenoVersion"
$zip = Fetch $DenoUrl $DenoSha "deno-$DenoVersion-$Triple.zip"
$dir = Join-Path $Build 'deno'
Fresh $dir
Expand-Archive -LiteralPath $zip -DestinationPath $dir -Force
Copy-Item -Force -LiteralPath (Join-Path $dir 'deno.exe') -Destination (Join-Path $Out "deno-$Triple.exe")

# ── fpcalc ──
Log "fpcalc (chromaprint $ChromaprintVersion)"
$zip = Fetch $FpcalcUrl $FpcalcSha "fpcalc-$ChromaprintVersion-windows-x86_64.zip"
$dir = Join-Path $Build 'fpcalc'
Fresh $dir
Expand-Archive -LiteralPath $zip -DestinationPath $dir -Force
$bin = Get-ChildItem -LiteralPath $dir -Recurse -File -Filter 'fpcalc.exe' | Select-Object -First 1
if (-not $bin) { Fail 'в архиве chromaprint нет fpcalc.exe' }
Copy-Item -Force -LiteralPath $bin.FullName -Destination (Join-Path $Out "fpcalc-$Triple.exe")
$tgz = Fetch $ChromaprintSrcUrl $ChromaprintSrcSha "chromaprint-$ChromaprintVersion.tar.gz"
$lic = Join-Path $Out 'licenses/chromaprint'
New-Item -ItemType Directory -Force -Path $lic | Out-Null
& $Tar -xzf $tgz -C $lic --strip-components 1 "chromaprint-$ChromaprintVersion/LICENSE.md"

# ── ffmpeg и ffprobe: готовая LGPL-сборка из ffmpeg-windows.sh ──
Log 'ffmpeg и ffprobe'
Copy-Item -Force -LiteralPath (Join-Path $Ffmpeg 'ffmpeg.exe') -Destination (Join-Path $Out "ffmpeg-$Triple.exe")
Copy-Item -Force -LiteralPath (Join-Path $Ffmpeg 'ffprobe.exe') -Destination (Join-Path $Out "ffprobe-$Triple.exe")
$license = (& (Join-Path $Out "ffmpeg-$Triple.exe") -hide_banner -L) -join "`n"
if ($license -notmatch 'Lesser General Public License') { Fail 'FFmpeg собран не под LGPL' }
$lic = Join-Path $Out 'licenses/ffmpeg'
New-Item -ItemType Directory -Force -Path $lic | Out-Null
foreach ($name in 'COPYING.LGPLv2.1', 'LICENSE.md', 'BUILD.txt') {
  $file = Join-Path $Ffmpeg $name
  if (Test-Path -LiteralPath $file) { Copy-Item -Force -LiteralPath $file -Destination $lic }
}

# ── сервер PO-токенов bgutil ──
Log "bgutil-server $BgutilVersion"
$bgutil = Fetch $BgutilUrl $BgutilSha "bgutil-ytdlp-pot-provider-$BgutilVersion.tar.gz"
$canvas = Fetch $CanvasUrl $CanvasSha "canvas-v$CanvasVersion-win32-x64.tar.gz"
$src = Join-Path $Build "bgutil-ytdlp-pot-provider-$BgutilVersion"
$dst = Join-Path $Build 'bgutil-server'
if (Test-Path -LiteralPath $src) { Remove-Item -Recurse -Force -LiteralPath $src }
Fresh $dst
& $Tar -xzf $bgutil -C $Build
Copy-Item -Recurse -LiteralPath (Join-Path $src 'server/src'), (Join-Path $src 'server/types') -Destination $dst
foreach ($name in 'package.json', 'package-lock.json', 'tsconfig.json') {
  Copy-Item -LiteralPath (Join-Path $src "server/$name") -Destination $dst
}
Copy-Item -LiteralPath (Join-Path $src 'LICENSE') -Destination (Join-Path $dst 'LICENSE')
# Только зависимости из package-lock и без их install-скриптов: сборка canvas заменена готовой ниже
Push-Location $dst
try { npm ci --omit=dev --ignore-scripts --no-audit --no-fund --loglevel=error } finally { Pop-Location }
$canvasDir = Join-Path $dst 'node_modules/canvas'
New-Item -ItemType Directory -Force -Path $canvasDir | Out-Null
& $Tar -xzf $canvas -C $canvasDir
if (-not (Test-Path -LiteralPath (Join-Path $canvasDir 'build/Release/canvas.node'))) { Fail 'в сборке canvas нет canvas.node' }
# Для запуска не нужны: карты исходников, типы TypeScript, описания, браузерная сборка youtubei.js
Get-ChildItem -LiteralPath (Join-Path $dst 'node_modules') -Recurse -File | Where-Object {
  $n = $_.Name
  $n -like '*.map' -or $n -like '*.d.ts' -or $n -like '*.d.mts' -or $n -like '*.d.cts' -or
  ($n -like '*.md' -and $n -notlike 'LICEN[CS]E*' -and $n -notlike 'COPYING*')
} | Remove-Item -Force
$bundle = Join-Path $dst 'node_modules/youtubei.js/bundle'
if (Test-Path -LiteralPath $bundle) { Remove-Item -Recurse -Force -LiteralPath $bundle }
$res = Join-Path $Out 'resources/bgutil-server'
if (Test-Path -LiteralPath $res) { Remove-Item -Recurse -Force -LiteralPath $res }
Copy-Item -Recurse -LiteralPath $dst -Destination $res
# Своя точка входа и deno.json: сервер выходит вместе с загрузчиком, lock-файл не пишется
Copy-Item -Force -LiteralPath (Join-Path $Here 'pot/serve.ts'), (Join-Path $Here 'pot/deno.json') -Destination $res
$lic = Join-Path $Out 'licenses/bgutil-ytdlp-pot-provider'
New-Item -ItemType Directory -Force -Path $lic | Out-Null
Copy-Item -Force -LiteralPath (Join-Path $dst 'LICENSE') -Destination (Join-Path $lic 'LICENSE')

# ── сам загрузчик: PyInstaller, один файл ──
Log "dalywave-fetcher: Python $PythonVersion, PyInstaller"
$env:UV_PYTHON_INSTALL_DIR = Join-Path $Build 'python'
uv python install --quiet $PythonVersion
$venv = Join-Path $Build 'venv'
$env:UV_PROJECT_ENVIRONMENT = $venv
Push-Location $Fetcher
try {
  # Пакет переустанавливается всегда: без этого uv оставит прежнюю копию той же версии
  uv sync --quiet --frozen --no-dev --group sidecar --no-editable --reinstall-package dalywave-fetcher --managed-python --python $PythonVersion
}
finally { Pop-Location }
$py = Join-Path $venv 'Scripts/python.exe'
$site = (& $py -I -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])').Trim()
# В окружении сборки — ровно исходники из src/
$mine = Join-Path $Fetcher 'src/dalywave_fetcher'
foreach ($file in Get-ChildItem -LiteralPath $mine -Recurse -File -Filter '*.py') {
  $rel = [IO.Path]::GetRelativePath($mine, $file.FullName)
  $installed = Join-Path (Join-Path $site 'dalywave_fetcher') $rel
  if (-not (Test-Path -LiteralPath $installed) -or (Sha256 $installed) -ne (Sha256 $file.FullName)) {
    Fail "в окружении сборки не тот dalywave_fetcher, что в src/ ($rel)"
  }
}
$plugin = (& $py -I -c 'import importlib.metadata as m; print(m.version("bgutil-ytdlp-pot-provider"))').Trim()
if ($plugin -ne $BgutilVersion) { Fail "плагин bgutil $plugin, а сервер $BgutilVersion — обновите BgutilVersion" }
# Плагины yt-dlp ищутся как папки на sys.path, а не в архиве модулей: кладём исходники рядом
$plugins = Join-Path $Build 'plugins'
Fresh $plugins
$pluginSrc = Join-Path $site 'yt_dlp_plugins'
foreach ($file in Get-ChildItem -LiteralPath $pluginSrc -Recurse -File -Filter '*.py') {
  $target = Join-Path (Join-Path $plugins 'yt_dlp_plugins') ([IO.Path]::GetRelativePath($pluginSrc, $file.FullName))
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
  Copy-Item -LiteralPath $file.FullName -Destination $target
}
$pyinstaller = Join-Path $venv 'Scripts/pyinstaller.exe'
# Консольная программа: протокол идёт через stdin и stdout. Окна консоли нет — приложение
# запускает её с CREATE_NO_WINDOW. Без UPX: сжатые exe чаще принимают за вирусы
& $pyinstaller --noconfirm --clean --log-level WARN --onefile --console --noupx `
  --name "dalywave-fetcher-$Triple" `
  --distpath $Out --workpath (Join-Path $Build 'pyinstaller') --specpath (Join-Path $Build 'pyinstaller') `
  --add-data "$(Join-Path $plugins 'yt_dlp_plugins');yt_dlp_plugins" `
  --collect-data ytmusicapi `
  --exclude-module fastapi --exclude-module uvicorn --exclude-module starlette `
  --exclude-module tkinter --exclude-module pytest `
  (Join-Path $Here 'entry.py')
$lic = Join-Path $Out 'licenses/python'
New-Item -ItemType Directory -Force -Path $lic | Out-Null
$pyLicense = Get-ChildItem -LiteralPath (Join-Path $Build 'python') -Recurse -File -Filter 'LICENSE.txt' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($pyLicense) { Copy-Item -Force -LiteralPath $pyLicense.FullName -Destination $lic }

# ── проверка: без токена загрузчик должен найти свои программы и выйти с кодом 2 ──
Log 'проверка'
$smoke = Join-Path $Build 'smoke'
Fresh $smoke
$exe = Join-Path $Out "dalywave-fetcher-$Triple.exe"
$info = [Diagnostics.ProcessStartInfo]::new($exe)
foreach ($arg in 'sidecar', '--data-dir', (Join-Path $smoke 'data'), '--log-file', (Join-Path $smoke 'fetcher.log'), '--no-pot') {
  $info.ArgumentList.Add($arg)
}
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
$info.Environment.Remove('DALYWAVE_DOWNLOADER_TOKEN') | Out-Null
$info.Environment.Remove('FETCHER_WORKER_TOKEN') | Out-Null
$proc = [Diagnostics.Process]::Start($info)
$proc.StandardInput.Close()
$stdout = $proc.StandardOutput.ReadToEndAsync()
$stderr = $proc.StandardError.ReadToEndAsync()
if (-not $proc.WaitForExit(120000)) { $proc.Kill($true); Fail 'проверочный запуск не закончился за 2 минуты' }
$proc.WaitForExit()
Set-Content -LiteralPath (Join-Path $smoke 'events.jsonl') -Value $stdout.Result
Set-Content -LiteralPath (Join-Path $smoke 'stderr.log') -Value $stderr.Result
if ($proc.ExitCode -ne 2) {
  Write-Host $stderr.Result
  Fail "проверочный запуск: код $($proc.ExitCode) вместо 2 (нет токена) — $smoke"
}
$helloLine = ($stdout.Result -split "`r?`n") | Where-Object { $_ -match '"event"\s*:\s*"hello"' } | Select-Object -First 1
if (-not $helloLine) { Fail "нет события hello — $smoke" }
$hello = $helloLine | ConvertFrom-Json
$outFull = [IO.Path]::GetFullPath($Out)
$broken = @()
foreach ($tool in $hello.tools.PSObject.Properties) {
  $path = [string] $tool.Value.path
  if (-not $tool.Value.version -or -not $path.StartsWith($outFull, [StringComparison]::OrdinalIgnoreCase)) { $broken += $tool.Name }
}
if ($broken.Count) { Fail "не найдены или не запускаются свои программы: $($broken -join ', ') — $helloLine" }

# ── паспорт сборки ──
$files = [ordered] @{}
foreach ($name in 'dalywave-fetcher', 'deno', 'ffmpeg', 'ffprobe', 'fpcalc') {
  $file = Join-Path $Out "$name-$Triple.exe"
  $files["$name-$Triple.exe"] = [ordered] @{ size = (Get-Item -LiteralPath $file).Length; sha256 = Sha256 $file }
}
$files["dalywave-fetcher-$Triple.exe"].version = $hello.version
$files["dalywave-fetcher-$Triple.exe"].python = $PythonVersion
$files["deno-$Triple.exe"].source = $DenoUrl
$files["fpcalc-$Triple.exe"].source = $FpcalcUrl
$files["ffmpeg-$Triple.exe"].license = 'LGPL-2.1-or-later (ffmpeg-windows.sh, без --enable-gpl и --enable-nonfree)'
$resFiles = Get-ChildItem -LiteralPath $res -Recurse -File
$files['resources/bgutil-server'] = [ordered] @{
  files = $resFiles.Count; size = ($resFiles | Measure-Object -Sum Length).Sum
  version = $BgutilVersion; source = $BgutilUrl; canvas = [ordered] @{ version = $CanvasVersion; source = $CanvasUrl }
}
$manifest = [ordered] @{
  target = $Triple
  builtAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  tools = $hello.tools
  files = $files
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Out 'MANIFEST.json') -Encoding utf8
foreach ($entry in $files.GetEnumerator()) {
  '{0,8:N1} МБ  {1}' -f ($entry.Value.size / 1e6), $entry.Key | Write-Host
}
Log "готово: $Out"
