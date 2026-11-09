#!/bin/sh

# POSIX sh fallback: prompt boundaries only, therefore history level 2.
HISTFILE=/dev/null
export HISTFILE
CM_INTEGRATION_SCRIPT=${ENV:-}
unset CM_NONCE
# Set in WSL tabs: the folder is reported as a Windows path (wslpath -w).
cm_wsl=${CM_WSL:-}
unset CM_WSL
# busybox ash (Alpine) decodes backslash escapes in PS1 after running the
# command substitution below, halving every backslash cm_precmd prints;
# bash and dash leave its output alone.
cm_prompt_bs=
case $(readlink /proc/$$/exe 2>/dev/null) in
  *busybox*) cm_prompt_bs=1 ;;
esac
cm_osc() { printf '\033]633;%s\a' "$1"; }
cm_precmd() {
  code=$?
  cm_osc "D;$code"
  cm_cwd=$PWD
  if [ -n "$cm_wsl" ]; then
    cm_cwd=$(wslpath -w "$PWD" 2>/dev/null) || cm_cwd=$PWD
  fi
  cm_cwd=$(printf '%s' "$cm_cwd" | sed 's/\\/\\\\/g; s/;/\\x3b/g')
  if [ -n "$cm_prompt_bs" ]; then
    cm_cwd=$(printf '%s' "$cm_cwd" | sed 's/\\/\\\\/g')
  fi
  cm_osc "P;Cwd=$cm_cwd"
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
