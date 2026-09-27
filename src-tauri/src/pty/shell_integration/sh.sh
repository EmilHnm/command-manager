#!/bin/sh

# POSIX sh fallback: prompt boundaries only, therefore history level 2.
HISTFILE=/dev/null
export HISTFILE
CM_INTEGRATION_SCRIPT=${ENV:-}
unset CM_NONCE
cm_osc() { printf '\033]633;%s\a' "$1"; }
cm_precmd() {
  code=$?
  cm_osc "D;$code"
  cm_osc "P;Cwd=$(printf '%s' "$PWD" | sed 's/\\/\\\\/g; s/;/\\x3b/g')"
  cm_osc A
}
cm_user_ps1=${PS1:-'\$ '}
CM_INPUT_MARKER=$(printf '\033]633;B\a')
# Keep the precmd command substitution, but append B after the user's prompt.
# The previous mixed quote expression was invalid in dash and never installed
# the integration prompt.
PS1='$(cm_precmd)'"$cm_user_ps1$CM_INPUT_MARKER"
if [ -n "$CM_INTEGRATION_SCRIPT" ]; then
  rm -f -- "$CM_INTEGRATION_SCRIPT" 2>/dev/null || true
  unset CM_INTEGRATION_SCRIPT
fi
