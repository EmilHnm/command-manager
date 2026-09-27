# OSC 633 / PowerShell spike

This scratchpad is deliberately outside the application modules. It tests the
first §11 risk: whether PowerShell + PSReadLine emits a usable OSC 633 stream
through the same portable-pty ConPTY path used by the application.

## Run

From the repository root:

    $cargo = 'C:\Users\HOA\.cargo\bin\cargo.exe'
    $env:CM_SPIKE_PWSH = (Get-Command pwsh).Source
    & $cargo run --manifest-path .\scratchpad\osc633-spike\Cargo.toml -- --script (Resolve-Path .\scratchpad\osc633-spike\shell_integration.ps1) --nonce spike-20260926

The harness writes osc633.raw and osc633.frames.txt next to the script.

Validate the captured lifecycle and nonce with:

    python .\scratchpad\osc633-spike\analyze.py --frames .\scratchpad\osc633-spike\osc633.frames.txt --nonce spike-20260926

## Protocol choices

- A, B, C, D and P;Cwd= use BEL (ESC ]633;... BEL) as the terminator.
- E escapes backslash, semicolon, ASCII control characters and spaces/newlines
  using the xAB form required by the OSC 633 convention.
- The nonce is passed through CM_NONCE and appended to E.
- The multiline case is intentionally included to observe how PSReadLine and
  ConPTY represent continuation input.
