# Command Manager zsh integration. Sourced inside the app-owned shell only.
export HISTFILE=/dev/null
export CM_SHELL_INTEGRATION=1
CM_APP_NONCE=${CM_NONCE:-}
unset CM_NONCE

cm_escape() {
  local value=$1 char out=
  for char in ${(s::)value}; do
    case "$char" in
      \\) out+='\\\\' ;;
      ';') out+='\\x3b' ;;
      $'\n'|$'\r'|$'\t'|$' ') printf -v char '%02x' "'$char"; out+="\\x$char" ;;
      *) out+=$char ;;
    esac
  done
  print -rn -- "$out"
}
cm_osc() { print -rn -- $'\033]633;'"$1"$'\a'; }
cm_preexec() {
  cm_osc "E;$(cm_escape "$1");$(cm_escape "$CM_APP_NONCE")"
  cm_osc C
}
cm_precmd() {
  local code=$?
  cm_osc "D;$code"
  cm_osc "P;Cwd=$(cm_escape "$PWD")"
  cm_osc A
}
precmd_functions+=(cm_precmd)
preexec_functions+=(cm_preexec)
PROMPT="${PROMPT:-%n@%m:%~%# }%{\e]633;B\a%}"
# Disable zsh-autosuggestions only for this app-owned shell. The application
# renders its own suggestions from command_history.
typeset -ga ZSH_AUTOSUGGEST_STRATEGY
ZSH_AUTOSUGGEST_STRATEGY=()
if (( $+functions[compinit] )); then
  :
fi
