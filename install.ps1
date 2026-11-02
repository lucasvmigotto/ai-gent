<#
.SYNOPSIS
  ai-gent installer for Windows: download a release package, verify its checksum,
  unpack it under a versioned directory and run its setup.ps1. The counterpart of install.sh.

.DESCRIPTION
  irm https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.ps1 | iex
  .\install.ps1 [-Version <X.Y.Z|latest>] [-Target claude,codex] [-List] [-Rollback] [-Prune]
                [-VerifyAttestation] [-SkipPrompt]

  Under the default execution policy a downloaded .ps1 file does not run; use the one-liner
  above, or: powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1

  Layout: $AI_GENT_HOME\<version>\ for each release, `current` -> the active one and `previous`
  -> the one before (for -Rollback), both directory junctions (no administrator rights, no
  Developer Mode). Nothing is written under $AI_GENT_HOME until the download passed its
  checksum, and the package is unpacked into .stage.<pid> before it takes its name.

  Environment:
    VERSION               latest (default) or x.y.z (x.y.z pins: no update notices); -Version wins
    AI_GENT_HOME          install root (default: %LOCALAPPDATA%\ai-gent)
    AI_GENT_RELEASE_BASE  where releases live (default: the GitHub releases URL of
                          lucasvmigotto/ai-gent): https://..., file:///C:/..., or a directory path.
                          <base>\<ver>\ai-gent-<ver>-<triple>.zip and SHA256SUMS; "latest" reads
                          <base>\LATEST.
    AI_GENT_TARGET        the triple to install instead of the detected one (tests):
                          x86_64-pc-windows-msvc or aarch64-pc-windows-msvc

  Exit codes: 0 success, 1 failure, 2 bad arguments, 4 verification failed.

  Everything runs inside functions and the last line calls them, so a partially
  downloaded script never executes. This file is ASCII on purpose: Windows PowerShell 5.1
  decodes a download without a charset as Latin-1.
#>
[CmdletBinding()]
param(
  [string]$Version = $env:VERSION,
  [string[]]$Target,
  [switch]$List,
  [switch]$Rollback,
  [switch]$Prune,
  [switch]$VerifyAttestation,
  [switch]$SkipPrompt
)

$script:Slug = 'lucasvmigotto/ai-gent'
$script:Triples = @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')

function Say([string]$Message) { Write-Host "ai-gent: $Message" }

# Fail "message" [code] -- stop the installer; the code becomes the exit code
function Fail([string]$Message, [int]$Code = 1) {
  $ex = New-Object System.Exception $Message
  $ex.Data['code'] = $Code
  throw $ex
}

function Test-Version([string]$Text) { return ($Text -match '^[0-9]+\.[0-9]+\.[0-9]+$') }

function Test-Link([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  return ($null -ne $item -and (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0))
}

# Get-LinkName <path> -- the directory name a junction points at (the version), or ''
function Get-LinkName([string]$Path) {
  if (-not (Test-Link $Path)) { return '' }
  $item = Get-Item -LiteralPath $Path -Force
  $t = @($item.Target) | Where-Object { $_ } | Select-Object -First 1
  if (-not $t) { return '' }
  return (Split-Path -Leaf ([string]$t).TrimEnd('\', '/'))
}

function Remove-Link([string]$Path) {
  # not recursive: removes the junction itself, never what it points at
  [IO.Directory]::Delete($Path, $false)
}

# Set-Link <link> <target dir> -- point a junction at a directory. A junction can't be renamed
# over another, so the flip is delete-then-create (FR-020); `previous` survives a failure.
function Set-Link([string]$Link, [string]$TargetDir) {
  $old = Get-LinkName $Link
  if (Test-Link $Link) { Remove-Link $Link }
  elseif (Test-Path -LiteralPath $Link) { Fail "$Link exists and is not a link; move it aside and run the installer again" }
  try {
    New-Item -ItemType Junction -Path $Link -Value $TargetDir | Out-Null
  } catch {
    if ($old) {
      try { New-Item -ItemType Junction -Path $Link -Value (Join-Path (Split-Path -Parent $Link) $old) | Out-Null } catch { $null = $_ }
    }
    Fail "could not point $Link at $TargetDir : $($_.Exception.Message)"
  }
}

function Get-InstalledVersions([string]$Root) {
  if (-not (Test-Path -LiteralPath $Root -PathType Container)) { return @() }
  $names = Get-ChildItem -LiteralPath $Root -Directory -Force |
    Where-Object { (($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) -and (Test-Version $_.Name) } |
    ForEach-Object { $_.Name }
  return @($names | Sort-Object { [version]$_ })
}

# Get-Triple -- this platform's triple (or AI_GENT_TARGET), before anything is downloaded
function Get-Triple {
  if ($env:AI_GENT_TARGET) {
    if ($script:Triples -contains $env:AI_GENT_TARGET) { return $env:AI_GENT_TARGET }
    Fail "unsupported AI_GENT_TARGET '$($env:AI_GENT_TARGET)'; supported: $($script:Triples -join ' ')" 2
  }
  $arch = ''
  try { $arch = [string][System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture } catch { $arch = '' }
  if (-not $arch) {
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
  }
  switch -Regex ($arch) {
    '^(X64|AMD64)$' { return 'x86_64-pc-windows-msvc' }
    '^ARM64$' { return 'aarch64-pc-windows-msvc' }
  }
  Fail "unsupported platform windows/$arch; supported: $($script:Triples -join ' ')"
}

# Get-Entry <SHA256SUMS> <file name> -- the checksum listed for it, or ''
function Get-Entry([string]$Sums, [string]$Name) {
  foreach ($line in (Get-Content -LiteralPath $Sums)) {
    $m = [regex]::Match($line, '^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$')
    if ($m.Success -and $m.Groups[2].Value -eq $Name) { return $m.Groups[1].Value.ToLowerInvariant() }
  }
  return ''
}

# Get-File <url or path> <destination> -- https, file:// or a directory path; never plain http
function Get-File([string]$Source, [string]$Dest) {
  if ($Source -match '^https://') {
    try {
      $saved = $ProgressPreference
      $ProgressPreference = 'SilentlyContinue' # the progress bar makes Windows PowerShell 5.1 downloads very slow
      Invoke-WebRequest -UseBasicParsing -Uri $Source -OutFile $Dest -TimeoutSec 300
    } catch {
      Fail "could not download $Source"
    } finally {
      $ProgressPreference = $saved
    }
    return
  }
  if ($Source -match '^[A-Za-z][A-Za-z0-9+.-]+://' -and $Source -notmatch '^file:') {
    Fail "refusing to download $Source : only https:// and file:// are allowed"
  }
  $path = $Source
  if ($Source -match '^file:') { $path = ([Uri]$Source).LocalPath }
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Fail "could not download $Source" }
  Copy-Item -LiteralPath $path -Destination $Dest -Force
}

# Join-Base <base> <child...> -- a URL (forward slashes) or a path (backslashes)
function Join-Base([string]$Base, [string[]]$Parts) {
  $sep = '\'
  if ($Base -match '^(https?|file):') { $sep = '/' }
  $out = $Base.TrimEnd('\', '/')
  foreach ($p in $Parts) { $out = $out + $sep + $p }
  return $out
}

function Get-LatestVersion([string]$Base, [string]$Work) {
  if ($env:AI_GENT_RELEASE_BASE) {
    $f = Join-Path $Work 'LATEST'
    Get-File (Join-Base $Base @('LATEST')) $f
    return ((Get-Content -LiteralPath $f -Raw) -replace '[\s]', '')
  }
  $url = ''
  try {
    $saved = $ProgressPreference
    $ProgressPreference = 'SilentlyContinue'
    $r = Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/$($script:Slug)/releases/latest" -Method Head -TimeoutSec 30
    try { $url = $r.BaseResponse.ResponseUri.AbsoluteUri } catch { $url = '' }
    if (-not $url) { try { $url = $r.BaseResponse.RequestMessage.RequestUri.AbsoluteUri } catch { $url = '' } }
  } catch {
    Fail 'could not resolve the latest release; pass -Version x.y.z'
  } finally {
    $ProgressPreference = $saved
  }
  if (-not $url) { Fail 'could not resolve the latest release; pass -Version x.y.z' }
  return $url.TrimEnd('/').Split('/')[-1]
}

# Test-ZipEntries -- every entry name, before anything is written
function Test-ZipEntries($Zip, [string]$Name) {
  $top = $null
  foreach ($e in $Zip.Entries) {
    $n = $e.FullName.Replace('\', '/')
    $parts = $n.Split('/')
    if ($n.StartsWith('/') -or $n -match '^[A-Za-z]:' -or $n.Contains(':') -or ($parts -contains '..')) {
      Fail "$Name has unsafe paths ($($e.FullName)); nothing was installed" 4
    }
    if ($null -eq $top) { $top = $parts[0] }
    if ($parts[0] -ne $top -or -not $top) {
      Fail "$Name has entries outside its top directory ($($e.FullName)); nothing was installed" 4
    }
  }
  if (-not $top) { Fail "$Name is empty; nothing was installed" 4 }
  return $top
}

# Expand-Package <zip> <stage dir> -- extract below the archive's top directory, entry by entry
function Expand-Package([string]$ZipPath, [string]$Stage, [string]$Name) {
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  try {
    $zip = [IO.Compression.ZipFile]::OpenRead($ZipPath)
  } catch {
    Fail "$Name is not a readable zip; nothing was installed" 4
  }
  try {
    $top = Test-ZipEntries $zip $Name
    $stageFull = [IO.Path]::GetFullPath($Stage).TrimEnd('\') + '\'
    foreach ($e in $zip.Entries) {
      $rel = $e.FullName.Replace('\', '/')
      if ($rel.Length -le $top.Length + 1) { continue } # the top directory itself
      $rel = $rel.Substring($top.Length + 1)
      $dest = [IO.Path]::GetFullPath((Join-Path $Stage ($rel.Replace('/', '\'))))
      if (-not $dest.StartsWith($stageFull, [StringComparison]::OrdinalIgnoreCase)) {
        Fail "$Name has unsafe paths ($($e.FullName)); nothing was installed" 4
      }
      if ($rel.EndsWith('/')) {
        [IO.Directory]::CreateDirectory($dest) | Out-Null
        continue
      }
      [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($dest)) | Out-Null
      $in = $e.Open()
      try {
        $out = [IO.File]::Create($dest)
        try { $in.CopyTo($out) } finally { $out.Dispose() }
      } finally {
        $in.Dispose()
      }
    }
  } finally {
    $zip.Dispose()
  }
}

# Invoke-Setup <root> <args...> -- the package's own setup.ps1, through `current`, in a child
# process with the execution policy bypassed for it alone (the default policy refuses files).
function Invoke-Setup([string]$Root, [string[]]$SetupArgs) {
  $setup = Join-Path $Root 'current\setup.ps1'
  if (-not (Test-Path -LiteralPath $setup -PathType Leaf)) { Fail "$setup not found" }
  $hostExe = (Get-Process -Id $PID).Path
  if (-not $hostExe) { $hostExe = 'powershell' }
  & $hostExe -NoProfile -ExecutionPolicy Bypass -File $setup @SetupArgs | Out-Host
  if ($LASTEXITCODE -ne 0) { Fail "setup.ps1 failed (exit $LASTEXITCODE)" }
}

function Invoke-List([string]$Root) {
  if (-not (Test-Path -LiteralPath $Root -PathType Container)) { Say "nothing installed in $Root"; return }
  $cur = Get-LinkName (Join-Path $Root 'current')
  $prev = Get-LinkName (Join-Path $Root 'previous')
  foreach ($v in (Get-InstalledVersions $Root)) {
    $mark = ''
    if ($v -eq $cur) { $mark = ' (current)' }
    if ($v -eq $prev) { $mark = ' (previous)' }
    Write-Host "$v$mark"
  }
}

function Invoke-Prune([string]$Root) {
  $cur = Get-LinkName (Join-Path $Root 'current')
  $prev = Get-LinkName (Join-Path $Root 'previous')
  if (-not $cur) { Fail "no current version in $Root; nothing is pruned" }
  foreach ($v in (Get-InstalledVersions $Root)) {
    if ($v -eq $cur -or $v -eq $prev) { continue }
    Say "removing $(Join-Path $Root $v)"
    Remove-Item -LiteralPath (Join-Path $Root $v) -Recurse -Force
  }
  Get-ChildItem -LiteralPath $Root -Directory -Force -Filter '.stage.*' -ErrorAction SilentlyContinue |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
}

function Invoke-Rollback([string]$Root, [string[]]$SetupArgs) {
  $prev = Get-LinkName (Join-Path $Root 'previous')
  $cur = Get-LinkName (Join-Path $Root 'current')
  if (-not $prev -or -not (Test-Path -LiteralPath (Join-Path $Root $prev) -PathType Container)) {
    Fail "no previous version to roll back to in $Root"
  }
  if (-not (Test-Version $prev)) { Fail "unexpected previous link: $prev" }
  Say "rolling back: current $cur -> $prev (previous becomes $cur)"
  Set-Link (Join-Path $Root 'current') (Join-Path $Root $prev)
  if ($cur) { Set-Link (Join-Path $Root 'previous') (Join-Path $Root $cur) }
  Invoke-Setup $Root (@('-SkipPrompt') + $SetupArgs)
  Say "now on $prev"
}

function Invoke-Install([string]$Root, [string]$Base, [string]$Ver, [bool]$Explicit, [bool]$Attest, [string[]]$SetupArgs) {
  $work = Join-Path ([IO.Path]::GetTempPath()) ('ai-gent-install.' + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $work | Out-Null
  $stage = ''
  try {
    $triple = Get-Triple # an unsupported platform stops here, before any download
    if ($Ver -eq 'latest') { $Ver = Get-LatestVersion $Base $work }
    if (-not (Test-Version $Ver)) { Fail "not a release version: '$Ver' (want x.y.z or latest)" 2 }
    Say "installing ai-gent $Ver ($triple)"
    Say "  into  $(Join-Path $Root $Ver)  (current -> $Ver)"

    $verDir = Join-Path $Root $Ver
    if ((Test-Path -LiteralPath $verDir -PathType Container) -and -not (Test-Link $verDir)) {
      Say "$Ver is already unpacked"
    } else {
      $sums = Join-Path $work 'SHA256SUMS'
      Get-File (Join-Base $Base @($Ver, 'SHA256SUMS')) $sums
      $name = "ai-gent-$Ver-$triple.zip"
      if (-not (Get-Entry $sums $name)) {
        Fail "no archive for $triple in release $Ver (SHA256SUMS has no entry for $name); nothing was installed"
      }
      Say "  from  $(Join-Base $Base @($Ver, $name))"
      $zipPath = Join-Path $work $name
      Get-File (Join-Base $Base @($Ver, $name)) $zipPath
      $want = Get-Entry $sums $name
      $got = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
      if ($want -ne $got) { Fail "checksum mismatch for $name (expected $want, got $got); nothing was installed" 4 }
      Say "checksum ok ($got)"
      if ($Attest) {
        if (Get-Command gh -ErrorAction SilentlyContinue) {
          & gh attestation verify $zipPath -R $script:Slug *> $null
          if ($LASTEXITCODE -ne 0) { Fail 'attestation verification failed; nothing was installed' 4 }
          Say "attestation ok (gh attestation verify, $($script:Slug))"
        } else {
          Say 'gh is not installed: attestation NOT verified (checksum only)'
        }
      }

      # Look inside, then unpack beside the install, before anything takes its final name.
      New-Item -ItemType Directory -Path $Root -Force | Out-Null
      $stage = Join-Path $Root ".stage.$PID"
      if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
      New-Item -ItemType Directory -Path $stage | Out-Null
      Expand-Package $zipPath $stage $name
      $text = ''
      if (Test-Path -LiteralPath (Join-Path $stage 'VERSION')) { $text = (Get-Content -LiteralPath (Join-Path $stage 'VERSION') -Raw).Trim() }
      if ($text -ne $Ver) { Fail "the package says it is not $Ver; nothing was installed" }
      if (-not (Test-Path -LiteralPath (Join-Path $stage 'setup.ps1') -PathType Leaf)) { Fail 'the package has no setup.ps1; nothing was installed' }
      if (-not (Test-Path -LiteralPath (Join-Path $stage 'bin\ai-gent.exe') -PathType Leaf)) { Fail 'the package has no bin\ai-gent.exe; nothing was installed' }
      [IO.Directory]::Move($stage, $verDir)
      $stage = ''
    }

    $cur = Get-LinkName (Join-Path $Root 'current')
    if ($cur -ne $Ver) {
      if ($cur) { Set-Link (Join-Path $Root 'previous') (Join-Path $Root $cur) }
      Set-Link (Join-Path $Root 'current') $verDir
    }
    # An explicit version pins (update-check stays silent); latest or none clears it.
    $pinned = Join-Path $Root 'pinned'
    if ($Explicit) { [IO.File]::WriteAllText($pinned, "$Ver`n") } else { Remove-Item -LiteralPath $pinned -Force -ErrorAction SilentlyContinue }
    Invoke-Setup $Root $SetupArgs
    Say "installed $Ver in $Root; to update run this installer again, to undo: .\install.ps1 -Rollback"
  } finally {
    if ($stage) { Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
  }
}

# Install-AiGent -- the whole installer; returns the exit code and never calls `exit`
# (under `irm | iex` that would close the user's terminal).
function Install-AiGent([string]$Ver, [string[]]$Targets, [bool]$DoList, [bool]$DoRollback, [bool]$DoPrune, [bool]$Attest) {
  $ErrorActionPreference = 'Stop'
  try {
    try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { $null = $_ }
    if (-not $Ver) { $Ver = 'latest' }
    if ($Ver -ne 'latest' -and -not (Test-Version $Ver)) { Fail "not a release version: '$Ver' (want x.y.z or latest)" 2 }
    if (@(@($DoList, $DoRollback, $DoPrune) | Where-Object { $_ }).Count -gt 1) { Fail '-List, -Rollback and -Prune are exclusive' 2 }

    $root = $env:AI_GENT_HOME
    if (-not $root) {
      $local = $env:LOCALAPPDATA
      if (-not $local) { $local = Join-Path $HOME 'AppData\Local' }
      $root = Join-Path $local 'ai-gent'
    }
    $base = $env:AI_GENT_RELEASE_BASE
    if (-not $base) { $base = "https://github.com/$($script:Slug)/releases/download" }
    if ((Test-Path -LiteralPath $root) -and -not (Test-Path -LiteralPath $root -PathType Container)) {
      Fail "$root exists and is not a directory"
    }

    $setupArgs = @()
    if ($Targets) { $setupArgs += @('-Target', ($Targets -join ',')) }

    if ($DoList) { Invoke-List $root }
    elseif ($DoPrune) { Invoke-Prune $root }
    elseif ($DoRollback) { Invoke-Rollback $root $setupArgs }
    else {
      $explicit = ($Ver -ne 'latest')
      Invoke-Install $root $base $Ver $explicit $Attest (@('-SkipPrompt') + $setupArgs)
    }
    return 0
  } catch {
    $code = 1
    if ($_.Exception.Data -and $_.Exception.Data.Contains('code')) { $code = [int]$_.Exception.Data['code'] }
    [Console]::Error.WriteLine("ai-gent: error: $($_.Exception.Message)")
    return $code
  }
}

$aiGentExit = @(Install-AiGent $Version $Target $List.IsPresent $Rollback.IsPresent $Prune.IsPresent $VerifyAttestation.IsPresent)[-1]
if ($MyInvocation.MyCommand.Path) { exit $aiGentExit }
elseif ($aiGentExit -ne 0) { $global:LASTEXITCODE = $aiGentExit }
