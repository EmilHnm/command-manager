param(
    [string]$Nonce = $env:CM_NONCE
)

if ([string]::IsNullOrEmpty($Nonce)) {
    $Nonce = 'command-manager'
}

$script:OscEscape = [char]0x1b
$script:OscBel = [char]0x07

function global:CM-EscapeOscValue([AllowNull()][string]$Value) {
    if ($null -eq $Value) {
        return ''
    }

    $builder = [System.Text.StringBuilder]::new()
    foreach ($character in $Value.ToCharArray()) {
        $code = [int][char]$character
        if ($character -eq '\') {
            [void]$builder.Append('\\')
        } elseif ($character -eq ';') {
            [void]$builder.Append('\x3b')
        } elseif ($code -le 0x20) {
            [void]$builder.Append(('\x{0:x2}' -f $code))
        } else {
            [void]$builder.Append($character)
        }
    }
    $builder.ToString()
}

function global:CM-WriteOsc633([string]$Payload) {
    [Console]::Write("$($script:OscEscape)]633;$Payload$($script:OscBel)")
}

try {
    Import-Module PSReadLine -ErrorAction SilentlyContinue
    Set-PSReadLineOption -HistorySaveStyle SaveNothing
    Set-PSReadLineOption -PredictionSource None
} catch {
    # PSReadLine is optional in the fallback shell.
}

$function:CMOriginalPrompt = $function:prompt
$global:CMLastExitCode = 0

function global:prompt {
    # Capture $? before running any prompt work. LASTEXITCODE is only useful
    # for a failed native process; it is otherwise stale after a cmdlet.
    $lastCommandSucceeded = [bool]$?
    $exitCode = if ($lastCommandSucceeded) {
        0
    } elseif ($null -ne $global:LASTEXITCODE -and [int]$global:LASTEXITCODE -ne 0) {
        [int]$global:LASTEXITCODE
    } else {
        1
    }
    $global:CMLastExitCode = $exitCode

    CM-WriteOsc633("D;$exitCode")
    try {
        CM-WriteOsc633("P;Cwd=$(CM-EscapeOscValue ((Get-Location).Path))")
    } catch {
        # A non-file-system provider does not have a path suitable for Cwd.
    }
    CM-WriteOsc633('A')

    $promptText = & $function:CMOriginalPrompt
    # B must be part of the returned prompt text. Writing it before returning
    # the prompt makes the marker precede the visible prompt in a PTY.
    "$promptText$($script:OscEscape)]633;B$($script:OscBel)"
}

if (Get-Module -Name PSReadLine) {
    Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
        param($key, $arg)

        $commandLine = ''
        $cursor = 0
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState(
            [ref]$commandLine,
            [ref]$cursor
        )
        CM-WriteOsc633("E;$(CM-EscapeOscValue $commandLine);$(CM-EscapeOscValue $Nonce)")
        CM-WriteOsc633('C')
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}

# The script file is shared by every PowerShell tab and deliberately kept on
# disk: Windows rescans a newly created script passed to -File, which cost
# about 0.9 s per terminal. The per-session secret is the nonce, not the file.
Remove-Item Env:CM_NONCE -ErrorAction SilentlyContinue
