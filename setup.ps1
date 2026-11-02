<#
.SYNOPSIS
  Install this ai-gent tree's skills and plugins for Claude Code and Codex on Windows, and put
  its binary on the user PATH. The Windows counterpart of setup.sh, tiers 1-2 only (spec 032
  FR-022); the other agents of setup.sh need WSL or Git Bash.

.DESCRIPTION
  .\setup.ps1 [-Target claude,codex|all] [-DryRun] [-Uninstall] [-Force] [-NoPath] [-SkipPrompt]

  Claude Code  directory junctions in %USERPROFILE%\.claude\skills\<name> -> <this tree>\plugins\<name>
               (and skills\<name> when the tree has them). Junctions need no administrator rights.
  Codex        generated <plugin>-<skill>\SKILL.md entries in %USERPROFILE%\.agents\skills that
               point at the source file in this tree (Cursor, Copilot and Amp read the same
               directory but are not installed here), marked .ai-gent-generated.
  PATH         <this tree>\bin is added to the user PATH once. When this script runs from
               <root>\current, the entry is <root>\current\bin and survives upgrades.

  The choice of agents is kept in %LOCALAPPDATA%\ai-gent\state\targets for the next run
  (default: claude). -Uninstall removes only what this script made: links into this tree,
  generated entries, and (without -Target) the PATH entry.

  Environment (tests, odd setups): CLAUDE_SKILLS_DIR, AGENTS_SKILLS_DIR, AI_GENT_STATE_DIR,
  AI_GENT_NO_PATH=1 (same as -NoPath), AI_GENT_USER_PATH_FILE (a file standing in for the user
  PATH in the registry).

  Exit codes: 0 success, 1 failure, 2 bad arguments. ASCII on purpose (Windows PowerShell 5.1).
#>
[CmdletBinding()]
param(
  [string[]]$Target,
  [switch]$DryRun,
  [switch]$Uninstall,
  [switch]$Force,
  [switch]$NoPath,
  [switch]$SkipPrompt
)

$script:Marker = '.ai-gent-generated'
$script:AgentIds = @('claude', 'codex')
$script:Counts = @{}
$script:Skips = New-Object System.Collections.ArrayList

function Say([string]$Message) { Write-Host $Message }

function Fail([string]$Message, [int]$Code = 1) {
  $ex = New-Object System.Exception $Message
  $ex.Data['code'] = $Code
  throw $ex
}

function Add-Status([string]$Status, [string]$Name, [string]$Reason = '') {
  if (-not $script:Counts.ContainsKey($Status)) { $script:Counts[$Status] = New-Object System.Collections.ArrayList }
  [void]$script:Counts[$Status].Add($Name)
  if ($Status -eq 'skip') { [void]$script:Skips.Add("  skipped $Name : $Reason") }
}

function Write-Section([string]$Title, [string]$Path) {
  $script:Counts = @{}
  [void]$script:Skips.Clear()
  Say "$Title $Path"
}

function Write-SectionEnd {
  foreach ($k in ($script:Counts.Keys | Sort-Object)) {
    $names = $script:Counts[$k]
    if ($names.Count -le 3) { Say ("  [{0}] {1}" -f $k, ($names -join ', ')) } else { Say ("  [{0}] {1} items" -f $k, $names.Count) }
  }
  foreach ($s in $script:Skips) { Say $s }
}

function Test-Link([string]$Path) {
  $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
  return ($null -ne $item -and (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0))
}

function Get-LinkTarget([string]$Path) {
  if (-not (Test-Link $Path)) { return '' }
  $t = @((Get-Item -LiteralPath $Path -Force).Target) | Where-Object { $_ } | Select-Object -First 1
  if (-not $t) { return '' }
  return ([string]$t).TrimEnd('\', '/')
}

function Test-SamePath([string]$A, [string]$B) {
  return ($A.TrimEnd('\', '/') -ieq $B.TrimEnd('\', '/'))
}

function Remove-Link([string]$Path) { [IO.Directory]::Delete($Path, $false) }

function New-Link([string]$Link, [string]$TargetDir) {
  New-Item -ItemType Junction -Path $Link -Value $TargetDir | Out-Null
}

# Get-TreeRoot -- this tree, named through `current` when the script runs from the active version,
# so the links survive an upgrade
function Get-TreeRoot {
  $dir = $PSScriptRoot
  $leaf = Split-Path -Leaf $dir
  $parent = Split-Path -Parent $dir
  if ($leaf -match '^[0-9]+\.[0-9]+\.[0-9]+$' -and $parent) {
    $cur = Join-Path $parent 'current'
    $t = Get-LinkTarget $cur
    if ($t -and (Test-SamePath (Split-Path -Leaf $t) $leaf)) { return $cur }
  }
  return $dir
}

# Test-IntoTree <path> -- a link target inside this tree
function Test-IntoTree([string]$LinkTarget) {
  return ($LinkTarget -and $LinkTarget.StartsWith($script:Tree + '\', [StringComparison]::OrdinalIgnoreCase))
}

# Test-AiGentTarget <path> -- a link target that looks like another ai-gent tree's plugin or skill
function Test-AiGentTarget([string]$LinkTarget) {
  if (-not $LinkTarget) { return $false }
  $up = Split-Path -Parent (Split-Path -Parent $LinkTarget)
  return ($LinkTarget -match '[\\/](plugins|skills)[\\/][^\\/]+$' -and $up -and
    ((Test-Path -LiteralPath (Join-Path $up 'setup.ps1')) -or (Test-Path -LiteralPath (Join-Path $up 'setup.sh'))))
}

function Set-LinkTo([string]$Src, [string]$Dest) {
  $name = Split-Path -Leaf $Dest
  if (Test-Link $Dest) {
    $t = Get-LinkTarget $Dest
    if (Test-SamePath $t $Src) { Add-Status 'ok' $name; return }
    if (-not (Test-IntoTree $t) -and -not $Force) {
      Add-Status 'skip' $name "link to $t ; use -Force"
      return
    }
    Add-Status 'relink' $name
    if (-not $DryRun) { Remove-Link $Dest; New-Link $Dest $Src }
  } elseif (Test-Path -LiteralPath $Dest) {
    if (-not $Force) { Add-Status 'skip' $name 'a real file or directory is in the way ; use -Force to back it up'; return }
    $backup = "$Dest.bak-" + (Get-Date -Format 'yyyyMMddHHmmss')
    Add-Status 'backup' "$name -> $(Split-Path -Leaf $backup)"
    if (-not $DryRun) { Move-Item -LiteralPath $Dest -Destination $backup; New-Link $Dest $Src }
    Add-Status 'link' $name
  } else {
    Add-Status 'link' $name
    if (-not $DryRun) { New-Link $Dest $Src }
  }
}

# Remove-StaleLinks <dir> -- links into this tree whose source is gone, or all of them on -Uninstall
function Remove-StaleLinks([string]$Dir) {
  if (-not (Test-Path -LiteralPath $Dir -PathType Container)) { return }
  foreach ($item in (Get-ChildItem -LiteralPath $Dir -Force)) {
    if (-not (Test-Link $item.FullName)) { continue }
    $t = Get-LinkTarget $item.FullName
    if (-not (Test-IntoTree $t)) { continue }
    if ($Uninstall -or -not (Test-Path -LiteralPath $t)) {
      Add-Status 'unlink' $item.Name
      if (-not $DryRun) { Remove-Link $item.FullName }
    }
  }
}

function Get-Frontmatter([string]$File, [string]$Key) {
  $inside = $false
  $n = 0
  foreach ($line in (Get-Content -LiteralPath $File -Encoding UTF8)) {
    $n++
    if ($line.TrimEnd() -eq '---') {
      if ($n -eq 1) { $inside = $true; continue }
      break
    }
    if ($inside -and $line.StartsWith("${Key}: ")) { return $line.Substring($Key.Length + 2) }
  }
  return ''
}

function ConvertTo-YamlQuoted([string]$Text) {
  return '"' + $Text.Replace('\', '\\').Replace('"', '\"') + '"'
}

function Write-Utf8([string]$Path, [string]$Text) {
  [IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding $false))
}

# ------------------------------------------------------------------ Claude Code

function Install-Claude {
  Write-Section 'Claude Code:' $script:ClaudeDir
  if (-not $DryRun) { New-Item -ItemType Directory -Path $script:ClaudeDir -Force | Out-Null }
  Remove-StaleLinks $script:ClaudeDir
  if ($Uninstall) { Write-SectionEnd; return }
  foreach ($top in @('skills', 'plugins')) {
    $topDir = Join-Path $script:Tree $top
    if (-not (Test-Path -LiteralPath $topDir -PathType Container)) { continue }
    foreach ($d in (Get-ChildItem -LiteralPath $topDir -Directory)) {
      $marker = if ($top -eq 'skills') { Join-Path $d.FullName 'SKILL.md' } else { Join-Path $d.FullName '.claude-plugin\plugin.json' }
      if (Test-Path -LiteralPath $marker -PathType Leaf) {
        Set-LinkTo (Join-Path $topDir $d.Name) (Join-Path $script:ClaudeDir $d.Name)
      }
    }
  }
  Write-SectionEnd
}

# ------------------------------------------------------------------------ Codex

function Install-Entry([string]$SkillsDir, [string]$Name, [string]$Src) {
  $desc = Get-Frontmatter $Src 'description'
  if ($desc.Length -lt 1 -or $desc.Length -gt 1024) {
    Add-Status 'skip' $Name "description is $($desc.Length) chars ; agents need 1-1024"
    return
  }
  $dir = Join-Path $SkillsDir $Name
  if ((Test-Path -LiteralPath $dir) -and -not (Test-Path -LiteralPath (Join-Path $dir $script:Marker))) {
    Add-Status 'skip' $Name "exists and was not generated by this script"
    return
  }
  $srcDir = Split-Path -Parent $Src
  $lines = @(
    '---',
    "name: $Name",
    "description: $(ConvertTo-YamlQuoted $desc)",
    'metadata:',
    "  source: $(ConvertTo-YamlQuoted $Src)",
    '---',
    '',
    '<!-- Generated by ai-gent/setup.ps1. Edit the source file, not this one. -->',
    '',
    'The instructions for this skill live in the ai-gent repository. Read',
    ('`' + $Src + '` in full and follow it as if it were this skill.'),
    '',
    'While following it:',
    '',
    ('- Its base directory is `' + $srcDir + '`; resolve relative paths such'),
    '  as `../../references/pipeline.md` from there.',
    '- Skills written as `plugin:skill` (for example `devcontainer:setup`,',
    '  `frontend:spec`) are named `plugin-skill` here (`devcontainer-setup`,',
    '  `frontend-spec`); load them with the skill tool.',
    '- Where it mentions Claude Code tools (Read, Edit, Bash, Skill), use',
    "  this agent's equivalents."
  )
  $content = ($lines -join "`n") + "`n"
  $file = Join-Path $dir 'SKILL.md'
  $current = ''
  if (Test-Path -LiteralPath $file) { $current = [IO.File]::ReadAllText($file) }
  if ($current -ceq $content) { Add-Status 'ok' $Name; return }
  Add-Status 'generate' $Name
  if (-not $DryRun) {
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Write-Utf8 $file $content
    Write-Utf8 (Join-Path $dir $script:Marker) ''
  }
}

function Remove-StaleEntries([string]$SkillsDir) {
  if (-not (Test-Path -LiteralPath $SkillsDir -PathType Container)) { return }
  foreach ($d in (Get-ChildItem -LiteralPath $SkillsDir -Directory)) {
    if (-not (Test-Path -LiteralPath (Join-Path $d.FullName $script:Marker))) { continue }
    $src = ''
    $file = Join-Path $d.FullName 'SKILL.md'
    if (Test-Path -LiteralPath $file) {
      foreach ($line in (Get-Content -LiteralPath $file -Encoding UTF8)) {
        $m = [regex]::Match($line, '^  source: "(.*)"$')
        if ($m.Success) { $src = $m.Groups[1].Value.Replace('\"', '"').Replace('\\', '\'); break }
      }
    }
    # only entries made from this tree (or a tree that is gone) are ours to remove
    if ($Uninstall -or -not $src -or -not (Test-Path -LiteralPath $src -PathType Leaf)) {
      Add-Status 'remove' $d.Name
      if (-not $DryRun) { Remove-Item -LiteralPath $d.FullName -Recurse -Force }
    }
  }
}

function Install-Codex {
  Write-Section 'Codex:' $script:AgentsDir
  if ($Uninstall) { Remove-StaleEntries $script:AgentsDir; Write-SectionEnd; return }
  if (-not $DryRun) { New-Item -ItemType Directory -Path $script:AgentsDir -Force | Out-Null }
  Remove-StaleEntries $script:AgentsDir
  foreach ($top in @('skills')) {
    $topDir = Join-Path $script:Tree $top
    if (-not (Test-Path -LiteralPath $topDir -PathType Container)) { continue }
    foreach ($d in (Get-ChildItem -LiteralPath $topDir -Directory)) {
      $src = Join-Path $d.FullName 'SKILL.md'
      if (Test-Path -LiteralPath $src -PathType Leaf) { Install-Entry $script:AgentsDir $d.Name $src }
    }
  }
  $plugins = Join-Path $script:Tree 'plugins'
  if (Test-Path -LiteralPath $plugins -PathType Container) {
    foreach ($p in (Get-ChildItem -LiteralPath $plugins -Directory)) {
      if (-not (Test-Path -LiteralPath (Join-Path $p.FullName '.claude-plugin\plugin.json'))) { continue }
      $skills = Join-Path $p.FullName 'skills'
      if (-not (Test-Path -LiteralPath $skills -PathType Container)) { continue }
      foreach ($s in (Get-ChildItem -LiteralPath $skills -Directory)) {
        $src = Join-Path $s.FullName 'SKILL.md'
        if (Test-Path -LiteralPath $src -PathType Leaf) { Install-Entry $script:AgentsDir "$($p.Name)-$($s.Name)" $src }
      }
    }
  }
  Write-SectionEnd
  Say 'the git, db, secrets and repo guards are instructions only here: Codex does not run Claude Code hooks'
}

# ---------------------------------------------------------------------- user PATH

function Get-UserPath {
  if ($env:AI_GENT_USER_PATH_FILE) {
    if (Test-Path -LiteralPath $env:AI_GENT_USER_PATH_FILE) { return [IO.File]::ReadAllText($env:AI_GENT_USER_PATH_FILE).Trim() }
    return ''
  }
  $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
  if ($null -eq $key) { return '' }
  try { return [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } finally { $key.Dispose() }
}

function Send-EnvironmentChange {
  try {
    if (-not ('AiGent.NativeMethods' -as [type])) {
      Add-Type -Namespace AiGent -Name NativeMethods -MemberDefinition '[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Auto)] public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint Msg, System.UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out System.UIntPtr lpdwResult);'
    }
    $result = [UIntPtr]::Zero
    [void][AiGent.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 2000, [ref]$result)
  } catch { $null = $_ }
}

function Set-UserPath([string]$Value) {
  if ($env:AI_GENT_USER_PATH_FILE) { Write-Utf8 $env:AI_GENT_USER_PATH_FILE $Value; return }
  $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
  try { $key.SetValue('Path', $Value, [Microsoft.Win32.RegistryValueKind]::ExpandString) } finally { $key.Dispose() }
  Send-EnvironmentChange
}

function Test-PathEntry([string]$Entry, [string]$Dir) {
  return ($Entry.Trim().TrimEnd('\') -ieq $Dir.TrimEnd('\')) -or
    ([Environment]::ExpandEnvironmentVariables($Entry).Trim().TrimEnd('\') -ieq $Dir.TrimEnd('\'))
}

function Install-Bin {
  $binDir = Join-Path $script:Tree 'bin'
  $exe = Join-Path $binDir 'ai-gent.exe'
  if ($Uninstall) {
    if ($script:TargetGiven) { return }
  } elseif (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    return
  }
  if ($NoPath -or $env:AI_GENT_NO_PATH) { Say 'Binary: user PATH left alone (-NoPath)'; return }
  Say "Binary: $binDir"
  $entries = @((Get-UserPath) -split ';' | Where-Object { $_.Trim() })
  $has = @($entries | Where-Object { Test-PathEntry $_ $binDir }).Count -gt 0
  if ($Uninstall) {
    if (-not $has) { return }
    Say '  [unlink] ai-gent on the user PATH'
    if (-not $DryRun) { Set-UserPath ((@($entries | Where-Object { -not (Test-PathEntry $_ $binDir) })) -join ';') }
    return
  }
  if ($has) { Say '  [ok] ai-gent on the user PATH'; return }
  Say '  [link] ai-gent on the user PATH (open a new terminal to use it)'
  if (-not $DryRun) { Set-UserPath ((@($entries) + $binDir) -join ';') }
}

# --------------------------------------------------------------------- selection

function Get-StateFile {
  $dir = $env:AI_GENT_STATE_DIR
  if (-not $dir) {
    $local = $env:LOCALAPPDATA
    if (-not $local) { $local = Join-Path $HOME 'AppData\Local' }
    $dir = Join-Path $local 'ai-gent\state'
  }
  return (Join-Path $dir 'targets')
}

function Get-PreviousAgents {
  $f = Get-StateFile
  $ids = @()
  if (Test-Path -LiteralPath $f) {
    $ids = @((Get-Content -LiteralPath $f -Raw) -split '[\s,]+' | Where-Object { $script:AgentIds -contains $_ })
  }
  if ($ids.Count -eq 0) { $ids = @('claude') }
  return $ids
}

function Resolve-Agents {
  if ($Uninstall -and -not $script:TargetGiven) { return $script:AgentIds }
  if (-not $script:TargetGiven) { return @(Get-PreviousAgents) }
  $chosen = @()
  foreach ($t in @((@($Target) -join ',').Split(',') | Where-Object { $_.Trim() })) {
    $t = $t.Trim().ToLowerInvariant()
    if ($t -eq 'all') { $chosen += $script:AgentIds; continue }
    if ($script:AgentIds -contains $t) { $chosen += $t; continue }
    if (@('opencode', 'cursor', 'antigravity', 'copilot', 'amp', 'kiro', 'windsurf') -contains $t) {
      Fail "'$t' is not installed natively on Windows (tiers 1-2: claude, codex); use WSL or Git Bash and setup.sh for it" 2
    }
    Fail "invalid -Target: $t (one of: all $($script:AgentIds -join ' '))" 2
  }
  return @($script:AgentIds | Where-Object { $chosen -contains $_ })
}

function Save-Agents([string[]]$Ids) {
  if ($DryRun) { return }
  $f = Get-StateFile
  $keep = @($Ids)
  if ($Uninstall) { $keep = @(Get-PreviousAgents | Where-Object { $Ids -notcontains $_ }) }
  if ($keep.Count -gt 0) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $f) -Force | Out-Null
    Write-Utf8 $f (($keep -join ' ') + "`n")
  } else {
    Remove-Item -LiteralPath $f -Force -ErrorAction SilentlyContinue
  }
}

function Invoke-Setup {
  $ErrorActionPreference = 'Stop'
  try {
    $script:Tree = Get-TreeRoot
    $userHome = $env:USERPROFILE
    if (-not $userHome) { $userHome = $HOME }
    $script:ClaudeDir = $env:CLAUDE_SKILLS_DIR
    if (-not $script:ClaudeDir) { $script:ClaudeDir = Join-Path $userHome '.claude\skills' }
    $script:AgentsDir = $env:AGENTS_SKILLS_DIR
    if (-not $script:AgentsDir) { $script:AgentsDir = Join-Path $userHome '.agents\skills' }
    $script:TargetGiven = [bool]($Target -and (@($Target) -join '').Trim())

    $agents = @(Resolve-Agents)
    foreach ($id in $agents) {
      if ($id -eq 'claude') { Install-Claude }
      if ($id -eq 'codex') { Install-Codex }
    }
    Install-Bin
    Save-Agents $agents
    if ($DryRun) { Say 'dry run - nothing was changed.' }
    elseif (-not $Uninstall) { Say 'done - start a new Claude Code or Codex session to pick up a changed set of skills.' }
    return 0
  } catch {
    $code = 1
    if ($_.Exception.Data -and $_.Exception.Data.Contains('code')) { $code = [int]$_.Exception.Data['code'] }
    [Console]::Error.WriteLine("setup.ps1: error: $($_.Exception.Message)")
    return $code
  }
}

$setupCode = @(Invoke-Setup)[-1]
exit $setupCode
