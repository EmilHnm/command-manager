param(
    [string]$Nonce = $env:CM_NONCE
)

if ([string]::IsNullOrEmpty($Nonce)) {
    $Nonce = 'osc633-spike'
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

$function:CMOriginalPrompt = $function:prompt
$global:CMLastExitCode = 0

try {
    Set-PSReadLineOption -HistorySaveStyle SaveNothing
    Set-PSReadLineOption -PredictionSource None
} catch {
    # PSReadLine is optional in the fallback shell.
}

function global:prompt {
    $exitCode = if ($null -ne $global:LASTEXITCODE) {
        $global:LASTEXITCODE
    } else {
        $global:CMLastExitCode
    }

    CM-WriteOsc633("D;$exitCode")
    try {
        CM-WriteOsc633("P;Cwd=$(CM-EscapeOscValue ((Get-Location).Path))")
    } catch {
        # A non-file-system provider does not have a path suitable for Cwd.
    }
    CM-WriteOsc633('A')

    $promptText = & $function:CMOriginalPrompt
    CM-WriteOsc633('B')
    $promptText
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
