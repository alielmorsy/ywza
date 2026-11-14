//! Integration scripts. Each emits OSC 133 A (prompt), B (input), C (output), D;exit and OSC 7 (cwd)
//! after the user's own profile has run, wrapping rather than replacing their prompt.

pub const BASH: &str = r##"[ -f ~/.bashrc ] && . ~/.bashrc
__ywza_first=1
__ywza_ec=0
__ywza_nl=$'\n'
__ywza_prompt() {
  local p=$PWD
  [[ $OSTYPE == msys* || $OSTYPE == cygwin* ]] && p=$(pwd -W 2>/dev/null || echo "$PWD")
  p=${p//\%/%25}; p=${p// /%20}
  [[ $p == /* ]] || p=/$p
  [ -z "$__ywza_first" ] && printf '\e]133;D;%s\a' "$__ywza_ec"
  __ywza_first=
  printf '\e]7;file://%s%s\a\e]133;A\a' "$HOSTNAME" "$p"
  [[ $PS1 == *133\;B* ]] || PS1+='\[\e]133;B\a\]'
  [[ $PS0 == *133\;C* ]] || PS0+='\e]133;C\a'
}
if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then
  PROMPT_COMMAND=('__ywza_ec=$?' "${PROMPT_COMMAND[@]}" __ywza_prompt)
else
  PROMPT_COMMAND='__ywza_ec=$?'$__ywza_nl"${PROMPT_COMMAND:+$PROMPT_COMMAND$__ywza_nl}"'__ywza_prompt'
fi
"##;

pub const ZSHENV: &str = r##"[[ -f $YWZA_ZDOTDIR_USER/.zshenv ]] && source $YWZA_ZDOTDIR_USER/.zshenv
[[ $ZDOTDIR == $YWZA_ZDOTDIR_OURS ]] || YWZA_ZDOTDIR_USER=$ZDOTDIR
ZDOTDIR=$YWZA_ZDOTDIR_OURS
"##;

pub const ZSHRC: &str = r##"ZDOTDIR=$YWZA_ZDOTDIR_USER
[[ -f $ZDOTDIR/.zshrc ]] && source $ZDOTDIR/.zshrc
__ywza_ran=
__ywza_d() { local ec=$?; [[ -n $__ywza_ran ]] && printf '\e]133;D;%s\a' $ec; __ywza_ran=; return 0 }
__ywza_a() {
  local p=${PWD//\%/%25}; p=${p// /%20}
  printf '\e]7;file://%s%s\a\e]133;A\a' $HOST $p
  [[ $PS1 == *133\;B* ]] || PS1+=$'%{\e]133;B\a%}'
}
__ywza_c() { __ywza_ran=1; printf '\e]133;C\a' }
precmd_functions=(__ywza_d $precmd_functions __ywza_a)
preexec_functions+=(__ywza_c)
"##;

/// Passed as `-Command`; single quotes only so it survives any argument quoting.
/// Enter is rebound (when PSReadLine is there) because PowerShell has no preexec hook.
pub const PWSH: &str = r##"$global:__ywza_orig = $function:prompt
$global:__ywza_ran = $false
function global:prompt {
  $ok = $?
  $lec = $global:LASTEXITCODE
  $e = [string][char]27
  $b = [string][char]7
  $o = ''
  if ($global:__ywza_ran) {
    $code = if ($ok) { 0 } elseif ($lec -is [int] -and $lec -ne 0) { $lec } else { 1 }
    $o += $e + ']133;D;' + $code + $b
    $global:__ywza_ran = $false
  }
  if ($pwd.Provider.Name -eq 'FileSystem') {
    $p = $pwd.ProviderPath.Replace('\', '/').Replace('%', '%25').Replace(' ', '%20')
    if (-not $p.StartsWith('/')) { $p = '/' + $p }
    $o += $e + ']7;file://' + [Environment]::MachineName + $p + $b
  }
  [Console]::Write($o + $e + ']133;A' + $b)
  if (-not $ok) { Write-Error 'x' -ErrorAction Ignore }
  $r = & $global:__ywza_orig
  [string]$r + $e + ']133;B' + $b
}
if (-not (Get-Module PSReadLine)) { try { Import-Module PSReadLine -ErrorAction Stop } catch {} }
if (Get-Module PSReadLine) {
  Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
    $l = $null
    $c = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$l, [ref]$c)
    if ($l.Trim()) {
      $global:__ywza_ran = $true
      [Console]::Write([string][char]27 + ']133;C' + [string][char]7)
    }
    [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
  }
}
"##;
