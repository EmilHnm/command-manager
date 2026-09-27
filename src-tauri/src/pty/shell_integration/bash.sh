#!/usr/bin/env bash

# This file is sourced only by the shell process owned by Command Manager.
# It does not edit ~/.bashrc and disables on-disk shell history for this tab.
export HISTFILE=/dev/null
export CM_SHELL_INTEGRATION=1
CM_APP_NONCE=${CM_NONCE:-}
unset CM_NONCE
# The rcfile itself may already have a HISTCMD value. Start empty so the
# first user-entered readline buffer is observed rather than mistaken for a
# command already handled by the hook.
CM_LAST_HISTCMD=

if [[ -f "$HOME/.bashrc" && "${BASH_SOURCE[0]}" != "$HOME/.bashrc" ]]; then
  . "$HOME/.bashrc"
fi

cm_escape() {
  local value=$1 char code out= LC_ALL=C
  for ((i=0; i<${#value}; i++)); do
    char=${value:i:1}
    printf -v code '%d' "'${char}"
    if [[ $char == '\\' ]]; then out+='\\\\'
    elif [[ $char == ';' ]]; then out+='\\x3b'
    elif (( code <= 32 )); then printf -v code '%02x' "$code"; out+="\\x$code"
    else out+=$char
    fi
  done
  printf '%s' "$out"
}

cm_osc() { printf '\033]633;%s\a' "$1"; }

cm_preexec() {
  [[ ${CM_SHELL_INTEGRATION_ACTIVE:-0} == 1 ]] || return 0
  [[ ${BASH_COMMAND:-} == cm_* ]] && return 0
  case ${BASH_COMMAND:-} in
    'rm -f -- "${BASH_SOURCE[0]}"'*) return 0 ;;
  esac
  # DEBUG fires for every simple command in a compound line.  HISTCMD moves
  # once per readline buffer; it also stays unchanged for HISTCONTROL=ignorespace,
  # allowing us to avoid both duplicate E/C frames and privacy leaks.
  if [[ -n ${HISTCMD:-} ]]; then
    [[ ${HISTCMD} == ${CM_LAST_HISTCMD:-} ]] && return 0
    CM_LAST_HISTCMD=${HISTCMD}
  fi
  # DEBUG fires once per simple command.  Bash has already placed the
  # complete readline buffer in history at this point, so prefer `history 1`
  # to avoid recording only the last fragment of `one; two` or a pipeline.
  local line
  line=$(HISTTIMEFORMAT= builtin history 1 2>/dev/null)
  line="${line#*[0-9]}"
  line="${line#"${line%%[![:space:]]*}"}"
  [[ -n $line ]] || line=${BASH_COMMAND:-}
  cm_osc "E;$(cm_escape "$line");$(cm_escape "$CM_APP_NONCE")"
  cm_osc C
}

cm_precmd() {
  local code=$?
  cm_osc "D;$code"
  cm_osc "P;Cwd=$(cm_escape "$PWD")"
  cm_osc A
}

if [[ -n ${PROMPT_COMMAND:-} ]]; then
  PROMPT_COMMAND="cm_precmd;${PROMPT_COMMAND}"
else
  PROMPT_COMMAND=cm_precmd
fi
PS1="${PS1:-\\u@\\h:\\w\\$ }\[\033]633;B\a\]"
if declare -F bleopt >/dev/null 2>&1; then
  bleopt complete_auto_complete=off 2>/dev/null || true
fi
trap cm_preexec DEBUG
export CM_SHELL_INTEGRATION_ACTIVE=1

# The launcher owns this file. Remove it after the shell has loaded the
# functions so app-data does not accumulate one script per terminal session.
rm -f -- "${BASH_SOURCE[0]}" 2>/dev/null || true
