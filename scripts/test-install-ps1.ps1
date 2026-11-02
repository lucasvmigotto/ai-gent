<#
.SYNOPSIS
  Tests install.ps1 and setup.ps1 (spec 032, contracts/install-ps1.md) in a throwaway sandbox,
  with no network: packages built by scripts/package.py from the working tree, around a stub
  ai-gent.exe, are served from a directory (AI_GENT_RELEASE_BASE). Runs on Windows PowerShell 5.1
  and PowerShell 7, on Windows only (junctions); elsewhere it says so and exits 0. The scripts
  under test run in child processes, as `-ExecutionPolicy Bypass -File`, the way ai-gent update
  runs them. Run by the `windows` job of .github/workflows/check.yml (windows-2025 and
  windows-11-arm).

  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\test-install-ps1.ps1

  Needs python (python, python3 or py -3) for scripts/package.py. The sandbox lives under the
  temp directory and is removed at the end. The real user PATH is only touched on GitHub Actions
  (an ephemeral runner), and restored.
#>
$ErrorActionPreference = 'Stop'

if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) {
  Write-Host 'test-install-ps1: skipped (needs Windows: directory junctions)'
  exit 0
}

$Repo = Split-Path -Parent $PSScriptRoot
$HostExe = (Get-Process -Id $PID).Path
$Sandbox = Join-Path ([IO.Path]::GetTempPath()) ('ai-gent-test.' + [guid]::NewGuid().ToString('N').Substring(0, 8))
$Triples = @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')
$script:Total = 0
$script:Failed = 0

Write-Host "test-install-ps1: PowerShell $($PSVersionTable.PSVersion) ($($PSVersionTable.PSEdition)) in $Sandbox"

# ------------------------------------------------------------------ helpers

function Find-Python {
  foreach ($c in @('python3', 'python')) {
    $cmd = Get-Command $c -ErrorAction SilentlyContinue
    if ($cmd) {
      & $cmd.Source -c 'import sys; sys.exit(0 if sys.version_info >= (3, 8) else 1)' 2>$null
      if ($LASTEXITCODE -eq 0) { return @($cmd.Source) }
    }
  }
  $py = Get-Command py -ErrorAction SilentlyContinue
  if ($py) { return @($py.Source, '-3') }
  throw 'no python found (python3, python or py -3)'
}
$Python = Find-Python

function Quote-Arg([string]$Arg) {
  if ($Arg -ne '' -and $Arg -notmatch '[\s"]') { return $Arg }
  return '"' + $Arg.Replace('"', '\"') + '"'
}

# Invoke-Child <exe> <args...> -- run a process with the current environment; stdout, stderr and the code
function Invoke-Child([string]$Exe, [string[]]$ChildArgs) {
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $Exe
  $psi.Arguments = (($ChildArgs | ForEach-Object { Quote-Arg $_ }) -join ' ')
  $psi.UseShellExecute = $false
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.CreateNoWindow = $true
  $p = [System.Diagnostics.Process]::Start($psi)
  $o = $p.StandardOutput.ReadToEndAsync()
  $e = $p.StandardError.ReadToEndAsync()
  $p.WaitForExit()
  $r = New-Object psobject -Property @{ Code = $p.ExitCode; Out = $o.Result; Err = $e.Result }
  $r | Add-Member -NotePropertyName All -NotePropertyValue ($r.Out + "`n" + $r.Err)
  return $r
}

function Invoke-Script([string]$Script, [string[]]$ScriptArgs) {
  return (Invoke-Child $HostExe (@('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $Script) + $ScriptArgs))
}
function Install([string[]]$A) { $script:Last = Invoke-Script (Join-Path $Repo 'install.ps1') $A; return $script:Last }
function Setup([string[]]$A) { $script:Last = Invoke-Script (Join-Path $Home_ 'current\setup.ps1') $A; return $script:Last }

function Check([string]$Name, [scriptblock]$Body) {
  $script:Total++
  $ok = $false
  $why = ''
  try { $r = @(& $Body); $ok = ($r.Count -gt 0 -and $r[-1] -eq $true) } catch { $why = $_.Exception.Message }
  if ($ok) { Write-Host "      ok    $Name" }
  else {
    $script:Failed++
    Write-Host "      FAIL  $Name $why"
    if ($script:Last) { Write-Host ('            last output: ' + (($script:Last.All -replace '\s+', ' ').Trim())) }
  }
}

function Link-Name([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  if (-not $item -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) { return '' }
  $t = @($item.Target) | Where-Object { $_ } | Select-Object -First 1
  return (Split-Path -Leaf ([string]$t).TrimEnd('\', '/'))
}
function Is-Link([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  return ($null -ne $item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)
}

# Remove-Tree -- delete a tree without ever following a junction out of it
function Remove-Tree([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path)) { return }
  foreach ($l in @(Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 })) {
    try { [IO.Directory]::Delete($l.FullName, $false) } catch { $null = $_ }
  }
  Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue
}

function Reset-Env {
  Remove-Tree $Home_
  Remove-Tree $UserHome
  Remove-Item -LiteralPath $PathFile -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $UserHome | Out-Null
  Remove-Item Env:VERSION -ErrorAction SilentlyContinue
  Remove-Item Env:AI_GENT_TARGET -ErrorAction SilentlyContinue
  $env:AI_GENT_RELEASE_BASE = $Rel
}

# Build-Release <ver> -- one zip per Windows triple, SHA256SUMS and install.ps1 under $Rel\<ver>
function Build-Release([string]$Ver) {
  $dir = Join-Path $Rel $Ver
  New-Item -ItemType Directory -Path $dir -Force | Out-Null
  foreach ($t in $Triples) {
    $out = & $Python[0] @($Python[1..($Python.Count)] | Where-Object { $_ }) (Join-Path $Repo 'scripts\package.py') --version $Ver --out $dir --target $t --binary $Stub 2>&1
    if ($LASTEXITCODE -ne 0) { throw "package.py failed for $Ver $t : $out" }
  }
}

function Sha([string]$File) { return (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant() }

# ------------------------------------------------------------------ sandbox

New-Item -ItemType Directory -Path $Sandbox | Out-Null
$Home_ = Join-Path $Sandbox 'share dir\ai gent'  # spaces on purpose
$UserHome = Join-Path $Sandbox 'home dir'
$Rel = Join-Path $Sandbox 'release dir'
$Stub = Join-Path $Sandbox 'ai-gent.exe'
$PathFile = Join-Path $Sandbox 'user-path.txt'
[IO.File]::WriteAllBytes($Stub, [byte[]](0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00))
$Claude = Join-Path $UserHome '.claude\skills'
$Agents = Join-Path $UserHome '.agents\skills'

$saved = @{}
foreach ($n in @('AI_GENT_HOME', 'AI_GENT_RELEASE_BASE', 'AI_GENT_TARGET', 'VERSION', 'USERPROFILE', 'CLAUDE_SKILLS_DIR', 'AGENTS_SKILLS_DIR', 'AI_GENT_STATE_DIR', 'AI_GENT_USER_PATH_FILE', 'AI_GENT_NO_PATH', 'FAKE_GH_RC', 'PATH')) {
  $saved[$n] = [Environment]::GetEnvironmentVariable($n)
}
$env:AI_GENT_HOME = $Home_
$env:USERPROFILE = $UserHome
$env:CLAUDE_SKILLS_DIR = $Claude
$env:AGENTS_SKILLS_DIR = $Agents
$env:AI_GENT_STATE_DIR = Join-Path $UserHome 'state'
$env:AI_GENT_USER_PATH_FILE = $PathFile
Remove-Item Env:AI_GENT_NO_PATH -ErrorAction SilentlyContinue

try {
  New-Item -ItemType Directory -Path $UserHome | Out-Null
  $env:AI_GENT_RELEASE_BASE = $Rel
  foreach ($v in @('9.9.0', '9.9.1', '9.9.2')) { Build-Release $v }
  [IO.File]::WriteAllText((Join-Path $Rel 'LATEST'), "9.9.1`n")

  Write-Host '      -- the scripts'
  Check 'install.ps1 and setup.ps1 are ASCII (Windows PowerShell 5.1 reads a download as Latin-1)' {
    $bad = @('install.ps1', 'setup.ps1', 'scripts\test-install-ps1.ps1') | Where-Object { [IO.File]::ReadAllBytes((Join-Path $Repo $_)) | Where-Object { $_ -gt 127 } | Select-Object -First 1 }
    @($bad).Count -eq 0
  }
  Check 'both scripts parse' {
    $errs = $null
    foreach ($f in @('install.ps1', 'setup.ps1')) {
      [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $Repo $f), [ref]$null, [ref]$errs)
      if ($errs.Count -gt 0) { throw "$f : $($errs[0].Message)" }
    }
    $true
  }
  Check 'every Windows zip carries setup.ps1 and install.ps1, and the release lists install.ps1' {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    foreach ($t in $Triples) {
      $z = [IO.Compression.ZipFile]::OpenRead((Join-Path $Rel "9.9.0\ai-gent-9.9.0-$t.zip"))
      try {
        $names = @($z.Entries | ForEach-Object { $_.FullName })
        foreach ($n in @('setup.ps1', 'install.ps1', 'bin/ai-gent.exe', 'VERSION', 'MANIFEST')) {
          if ($names -notcontains "ai-gent-9.9.0-$t/$n") { throw "$t has no $n" }
        }
      } finally { $z.Dispose() }
    }
    (Get-Content -LiteralPath (Join-Path $Rel '9.9.0\SHA256SUMS')) -match '  install\.ps1$' | Select-Object -First 1 | ForEach-Object { $true }
  }

  Write-Host '      -- install, upgrade, rollback, list, prune, pin'
  Reset-Env
  Check 'a fresh install of 9.9.0 succeeds and verifies the checksum' {
    $r = Install @('-Version', '9.9.0', '-SkipPrompt')
    $r.Code -eq 0 -and $r.Out -match 'checksum ok' -and $r.Out -match 'installed 9\.9\.0'
  }
  Check 'current is a junction to 9.9.0 and the binary resolves through it' {
    (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.0' -and (Test-Path -LiteralPath (Join-Path $Home_ 'current\bin\ai-gent.exe') -PathType Leaf) -and
    (Test-Path -LiteralPath (Join-Path $Home_ '9.9.0\VERSION'))
  }
  Check 'an explicit version pins' { (Get-Content -LiteralPath (Join-Path $Home_ 'pinned') -Raw).Trim() -eq '9.9.0' }
  Check 'nothing is left staged or in the temp directory' {
    @(Get-ChildItem -LiteralPath $Home_ -Force -Filter '.stage.*').Count -eq 0 -and @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Filter 'ai-gent-install.*' -ErrorAction SilentlyContinue).Count -eq 0
  }
  Check 'the install is Claude Code junctions through current, and the user PATH names current\bin' {
    $link = Join-Path $Claude 'git'
    $t = @((Get-Item -LiteralPath $link -Force).Target)[0]
    (Is-Link $link) -and $t.StartsWith((Join-Path $Home_ 'current\plugins'), [StringComparison]::OrdinalIgnoreCase) -and
    (Test-Path -LiteralPath (Join-Path $link '.claude-plugin\plugin.json')) -and
    ((Get-Content -LiteralPath $PathFile -Raw) -split ';') -contains (Join-Path $Home_ 'current\bin')
  }
  Check 'the plugin reference files are regular files, not links or empty' {
    $f = @(Get-ChildItem -LiteralPath (Join-Path $Home_ '9.9.0\plugins\git\references') -File)
    $f.Count -gt 0 -and @($f | Where-Object { $_.Length -eq 0 -or ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 }).Count -eq 0
  }
  Check 'a second install of the same version is idempotent' {
    $r = Install @('-Version', '9.9.0', '-SkipPrompt')
    $r.Code -eq 0 -and $r.Out -match 'already unpacked' -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.0' -and -not (Test-Path -LiteralPath (Join-Path $Home_ 'previous'))
  }
  Check 'a latest install upgrades to 9.9.1, keeps 9.9.0 as previous and clears the pin' {
    $r = Install @('-SkipPrompt')
    $r.Code -eq 0 -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.1' -and (Link-Name (Join-Path $Home_ 'previous')) -eq '9.9.0' -and -not (Test-Path -LiteralPath (Join-Path $Home_ 'pinned'))
  }
  Check '-List marks current and previous' {
    $r = Install @('-List')
    $r.Code -eq 0 -and $r.Out -match '9\.9\.0 \(previous\)' -and $r.Out -match '9\.9\.1 \(current\)'
  }
  Check '-Rollback swaps current and previous, and again' {
    $r = Install @('-Rollback')
    $a = $r.Code -eq 0 -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.0' -and (Link-Name (Join-Path $Home_ 'previous')) -eq '9.9.1'
    $r = Install @('-Rollback')
    $a -and $r.Code -eq 0 -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.1'
  }
  Check 'the Claude Code junctions still resolve after the flips' { Test-Path -LiteralPath (Join-Path $Claude 'git\.claude-plugin\plugin.json') }
  Check '-Prune keeps current and previous and removes the rest' {
    [void](Install @('-Version', '9.9.2', '-SkipPrompt'))   # current 9.9.2, previous 9.9.1; 9.9.0 is now unreferenced
    $r = Install @('-Prune')
    $r.Code -eq 0 -and -not (Test-Path -LiteralPath (Join-Path $Home_ '9.9.0')) -and (Test-Path -LiteralPath (Join-Path $Home_ '9.9.1')) -and (Test-Path -LiteralPath (Join-Path $Home_ '9.9.2'))
  }
  Check '-Rollback with no previous refuses' {
    Reset-Env
    [void](Install @('-Version', '9.9.0', '-SkipPrompt'))
    $r = Install @('-Rollback')
    $r.Code -eq 1 -and $r.Err -match 'no previous version'
  }

  Write-Host '      -- refusals write nothing'
  Check 'a bad checksum exits 4, says so on stderr and installs nothing' {
    Reset-Env
    Build-Release '9.9.3'
    $zip = Join-Path $Rel '9.9.3\ai-gent-9.9.3-x86_64-pc-windows-msvc.zip'
    $fs = [IO.File]::Open($zip, 'Append')
    try { $fs.WriteByte(0) } finally { $fs.Dispose() }
    $env:AI_GENT_TARGET = 'x86_64-pc-windows-msvc'
    $r = Install @('-Version', '9.9.3', '-SkipPrompt')
    $r.Code -eq 4 -and $r.Err -match 'checksum mismatch' -and -not (Test-Path -LiteralPath $Home_)
  }
  Check 'an unsafe zip entry (..) exits 4 and installs nothing' {
    Reset-Env
    $dir = Join-Path $Rel '9.9.4'
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    $zip = Join-Path $dir 'ai-gent-9.9.4-x86_64-pc-windows-msvc.zip'
    $py = Join-Path $Sandbox 'evilzip.py'
    [IO.File]::WriteAllText($py, "import sys, zipfile`nwith zipfile.ZipFile(sys.argv[1], 'w') as z:`n    z.writestr('ai-gent-9.9.4-x86_64-pc-windows-msvc/VERSION', '9.9.4\n')`n    z.writestr('ai-gent-9.9.4-x86_64-pc-windows-msvc/../evil.txt', 'x')`n")
    & $Python[0] @($Python[1..($Python.Count)] | Where-Object { $_ }) $py $zip
    [IO.File]::WriteAllText((Join-Path $dir 'SHA256SUMS'), ((Sha $zip) + '  ai-gent-9.9.4-x86_64-pc-windows-msvc.zip' + "`n"))
    $env:AI_GENT_TARGET = 'x86_64-pc-windows-msvc'
    $r = Install @('-Version', '9.9.4', '-SkipPrompt')
    $r.Code -eq 4 -and $r.Err -match 'unsafe paths' -and -not (Test-Path -LiteralPath $Home_) -and -not (Test-Path -LiteralPath (Join-Path $Sandbox 'evil.txt'))
  }
  Check 'a release without an archive for this triple refuses' {
    Reset-Env
    $dir = Join-Path $Rel '9.9.5'
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $dir 'SHA256SUMS'), ('0' * 64) + "  other.zip`n")
    $r = Install @('-Version', '9.9.5', '-SkipPrompt')
    $r.Code -eq 1 -and $r.Err -match 'no archive for' -and -not (Test-Path -LiteralPath $Home_)
  }
  Check 'an unknown AI_GENT_TARGET exits 2 before any download' {
    Reset-Env
    $env:AI_GENT_TARGET = 'riscv64-unknown-linux-gnu'
    $r = Install @('-Version', '9.9.0', '-SkipPrompt')
    Remove-Item Env:AI_GENT_TARGET
    $r.Code -eq 2 -and $r.Err -match 'unsupported AI_GENT_TARGET' -and -not (Test-Path -LiteralPath $Home_)
  }
  Check 'a non-version exits 2' { (Install @('-Version', '..\x')).Code -eq 2 }
  Check 'plain http is refused as a release base' {
    Reset-Env
    $env:AI_GENT_RELEASE_BASE = 'http://example.invalid/releases'
    $r = Install @('-Version', '9.9.0', '-SkipPrompt')
    $env:AI_GENT_RELEASE_BASE = $Rel
    $r.Code -eq 1 -and $r.Err -match 'only https' -and -not (Test-Path -LiteralPath $Home_)
  }
  Check 'exclusive actions exit 2' { (Install @('-List', '-Prune')).Code -eq 2 }

  Write-Host '      -- attestation (a fake gh)'
  $fake = Join-Path $Sandbox 'fakebin'
  New-Item -ItemType Directory -Path $fake | Out-Null
  [IO.File]::WriteAllText((Join-Path $fake 'gh.cmd'), "@echo off`r`nexit /b %FAKE_GH_RC%`r`n")
  $env:PATH = $fake + ';' + $saved['PATH']
  Check '-VerifyAttestation passes when gh does' {
    Reset-Env
    $env:FAKE_GH_RC = '0'
    $r = Install @('-Version', '9.9.0', '-SkipPrompt', '-VerifyAttestation')
    $r.Code -eq 0 -and $r.Out -match 'attestation ok'
  }
  Check '-VerifyAttestation exits 4 and installs nothing when gh fails' {
    Reset-Env
    $env:FAKE_GH_RC = '1'
    $r = Install @('-Version', '9.9.0', '-SkipPrompt', '-VerifyAttestation')
    $r.Code -eq 4 -and $r.Err -match 'attestation verification failed' -and -not (Test-Path -LiteralPath (Join-Path $Home_ '9.9.0'))
  }
  $env:PATH = $saved['PATH']
  Remove-Item Env:FAKE_GH_RC -ErrorAction SilentlyContinue

  Write-Host '      -- other ways in'
  Check 'a file:// base works (and a path with spaces)' {
    Reset-Env
    $env:AI_GENT_RELEASE_BASE = ([Uri]$Rel).AbsoluteUri
    $r = Install @('-Version', '9.9.0', '-SkipPrompt')
    $env:AI_GENT_RELEASE_BASE = $Rel
    $r.Code -eq 0 -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.0'
  }
  Check 'piped into iex it installs, reads VERSION, and does not close the host' {
    Reset-Env
    $env:VERSION = '9.9.0'
    $script = (Join-Path $Repo 'install.ps1').Replace("'", "''")
    $r = Invoke-Child $HostExe @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', "iex ([IO.File]::ReadAllText('$script')); 'STILL-HERE'")
    $script:Last = $r
    Remove-Item Env:VERSION
    $r.Code -eq 0 -and $r.Out -match 'STILL-HERE' -and (Link-Name (Join-Path $Home_ 'current')) -eq '9.9.0'
  }
  Check 'piped into iex a failure does not close the host either' {
    Reset-Env
    $env:VERSION = '9.9.0'
    $env:AI_GENT_TARGET = 'riscv64-unknown-linux-gnu'
    $script = (Join-Path $Repo 'install.ps1').Replace("'", "''")
    $r = Invoke-Child $HostExe @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', "iex ([IO.File]::ReadAllText('$script')); 'STILL-HERE'")
    $script:Last = $r
    Remove-Item Env:VERSION
    Remove-Item Env:AI_GENT_TARGET
    $r.Out -match 'STILL-HERE' -and $r.Err -match 'unsupported AI_GENT_TARGET'
  }

  Write-Host '      -- setup.ps1'
  Reset-Env
  [void](Install @('-Version', '9.9.0', '-SkipPrompt'))
  Check '-DryRun changes nothing' {
    Remove-Tree $Claude
    Remove-Item -LiteralPath $PathFile -Force -ErrorAction SilentlyContinue
    $r = Setup @('-DryRun', '-Target', 'claude,codex')
    $r.Code -eq 0 -and $r.Out -match 'dry run' -and -not (Test-Path -LiteralPath $Claude) -and -not (Test-Path -LiteralPath $Agents) -and -not (Test-Path -LiteralPath $PathFile)
  }
  Check 'setup.ps1 links Claude Code, generates the Codex entries and adds the PATH entry once' {
    $r = Setup @('-Target', 'claude,codex')
    $r2 = Setup @('-Target', 'claude,codex')
    $entry = Join-Path $Agents 'git-workflow'
    $paths = @((Get-Content -LiteralPath $PathFile -Raw) -split ';' | Where-Object { $_ -eq (Join-Path $Home_ 'current\bin') })
    $r.Code -eq 0 -and $r2.Code -eq 0 -and (Is-Link (Join-Path $Claude 'git')) -and (Test-Path -LiteralPath (Join-Path $entry 'SKILL.md')) -and
    (Test-Path -LiteralPath (Join-Path $entry '.ai-gent-generated')) -and $paths.Count -eq 1 -and $r2.Out -match '\[ok\]'
  }
  Check 'a Codex entry names its source through current and carries the description' {
    $text = [IO.File]::ReadAllText((Join-Path $Agents 'git-workflow\SKILL.md'))
    $text -match '(?m)^name: git-workflow$' -and $text.Contains((Join-Path $Home_ 'current\plugins\git\skills\workflow\SKILL.md')) -and $text -match '(?m)^description: ".+"$' -and -not $text.Contains("`r")
  }
  Check 'the chosen agents are remembered' { (Get-Content -LiteralPath (Join-Path $UserHome 'state\targets') -Raw).Trim() -eq 'claude codex' }
  Check 'a link to somewhere else is left alone without -Force' {
    $other = Join-Path $Sandbox 'elsewhere'
    New-Item -ItemType Directory -Path $other | Out-Null
    $link = Join-Path $Claude 'db'
    [IO.Directory]::Delete($link, $false)
    New-Item -ItemType Junction -Path $link -Value $other | Out-Null
    $r = Setup @('-Target', 'claude')
    $kept = (Link-Name $link) -eq 'elsewhere'
    $r2 = Setup @('-Target', 'claude', '-Force')
    $r.Code -eq 0 -and $kept -and $r.Out -match 'use -Force' -and $r2.Code -eq 0 -and (Link-Name $link) -eq 'db'
  }
  Check 'a real directory in the way is skipped, then backed up with -Force' {
    $dir = Join-Path $Claude 'repo'
    [IO.Directory]::Delete($dir, $false)
    New-Item -ItemType Directory -Path $dir | Out-Null
    $r = Setup @('-Target', 'claude')
    $kept = -not (Is-Link $dir)
    $r2 = Setup @('-Target', 'claude', '-Force')
    $kept -and $r2.Code -eq 0 -and (Is-Link $dir) -and @(Get-ChildItem -LiteralPath $Claude -Filter 'repo.bak-*').Count -eq 1
  }
  Check 'a target setup.sh handles but Windows does not exits 2' { (Setup @('-Target', 'cursor')).Code -eq 2 }
  Check 'an unknown target exits 2' { (Setup @('-Target', 'nope')).Code -eq 2 }
  Check '-Uninstall -Target claude removes only the Claude Code links' {
    $r = Setup @('-Uninstall', '-Target', 'claude')
    $r.Code -eq 0 -and -not (Is-Link (Join-Path $Claude 'git')) -and (Test-Path -LiteralPath (Join-Path $Agents 'git-workflow\SKILL.md')) -and
    ((Get-Content -LiteralPath $PathFile -Raw) -split ';') -contains (Join-Path $Home_ 'current\bin') -and
    (Get-Content -LiteralPath (Join-Path $UserHome 'state\targets') -Raw).Trim() -eq 'codex'
  }
  Check '-Uninstall removes the entries, the PATH entry and the remembered choice, and keeps the install' {
    $r = Setup @('-Uninstall')
    $r.Code -eq 0 -and -not (Test-Path -LiteralPath (Join-Path $Agents 'git-workflow')) -and
    -not (((Get-Content -LiteralPath $PathFile -Raw) -split ';') -contains (Join-Path $Home_ 'current\bin')) -and
    (Test-Path -LiteralPath (Join-Path $Home_ '9.9.0\bin\ai-gent.exe')) -and -not (Test-Path -LiteralPath (Join-Path $UserHome 'state\targets')) -and
    -not (Is-Link (Join-Path $Claude 'repo'))
  }
  Check 'a PATH of other entries is kept as it is' {
    [IO.File]::WriteAllText($PathFile, 'C:\one;C:\two with space')
    [void](Setup @('-Target', 'claude'))
    $added = (Get-Content -LiteralPath $PathFile -Raw) -eq ('C:\one;C:\two with space;' + (Join-Path $Home_ 'current\bin'))
    [void](Setup @('-Uninstall'))
    $added -and (Get-Content -LiteralPath $PathFile -Raw) -eq 'C:\one;C:\two with space'
  }
  Check '-NoPath leaves the user PATH alone' {
    Remove-Item -LiteralPath $PathFile -Force -ErrorAction SilentlyContinue
    $r = Setup @('-Target', 'claude', '-NoPath')
    $r.Code -eq 0 -and -not (Test-Path -LiteralPath $PathFile)
  }

  if ($env:GITHUB_ACTIONS -eq 'true') {
    Write-Host '      -- the real user PATH (an ephemeral runner; restored afterwards)'
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
    $orig = $key.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    $origKind = if ($null -ne $orig) { $key.GetValueKind('Path') } else { $null }
    try {
      Remove-Item Env:AI_GENT_USER_PATH_FILE
      $bin = Join-Path $Home_ 'current\bin'
      Check 'the registry PATH gains current\bin once and keeps its type' {
        $key.SetValue('Path', '%USERPROFILE%\bin;C:\one', [Microsoft.Win32.RegistryValueKind]::ExpandString)
        [void](Setup @('-Target', 'claude'))
        [void](Setup @('-Target', 'claude'))
        $v = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $v -eq ('%USERPROFILE%\bin;C:\one;' + $bin) -and $key.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::ExpandString
      }
      Check 'a full -Uninstall takes it out again' {
        [void](Setup @('-Uninstall'))
        [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) -eq '%USERPROFILE%\bin;C:\one'
      }
    } finally {
      if ($null -eq $orig) { $key.DeleteValue('Path', $false) } else { $key.SetValue('Path', $orig, $origKind) }
      $key.Dispose()
      $env:AI_GENT_USER_PATH_FILE = $PathFile
    }
  }
} finally {
  foreach ($n in $saved.Keys) { [Environment]::SetEnvironmentVariable($n, $saved[$n]) }
  Remove-Tree $Sandbox
}

Write-Host ("test-install-ps1: {0} checks, {1} failed" -f $script:Total, $script:Failed)
if ($script:Failed -gt 0) { exit 1 }
exit 0
