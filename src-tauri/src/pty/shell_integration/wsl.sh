# Command Manager WSL launcher, run by `wsl.exe -e sh -c` inside the
# distribution. It starts the user's login shell with the matching shell
# integration, as WSL would start it without the app. The scripts are written
# on the Windows side; WSLENV (/p) hands their paths over as Linux paths.
# Every script is per tab: the unused ones are removed here.
cm_rm() { rm -rf -- "$@" 2>/dev/null; }
cm_shell=${SHELL:-}
[ -x "$cm_shell" ] || cm_shell=$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7)
[ -x "$cm_shell" ] || cm_shell=/bin/sh
case ${cm_shell##*/} in
  bash)
    cm_rm "$CM_SH_RC" "$CM_ZDOTDIR"
    if [ -r "$CM_BASH_RC" ]; then
      export CM_SHELL_LOGIN=1
      exec "$cm_shell" --rcfile "$CM_BASH_RC" -i
    fi
    ;;
  zsh)
    cm_rm "$CM_BASH_RC" "$CM_SH_RC"
    if [ -r "$CM_ZDOTDIR/.zshrc" ]; then
      export ZDOTDIR="$CM_ZDOTDIR"
      exec "$cm_shell" -il
    fi
    ;;
  sh | dash | ash)
    cm_rm "$CM_BASH_RC" "$CM_ZDOTDIR"
    if [ -r "$CM_SH_RC" ]; then
      export ENV="$CM_SH_RC"
      exec "$cm_shell" -il
    fi
    ;;
  *)
    cm_rm "$CM_BASH_RC" "$CM_SH_RC" "$CM_ZDOTDIR"
    ;;
esac
# fish and others, or the scripts could not be reached (automount off):
# the plain login shell, without integration.
unset CM_WSL
exec "$cm_shell" -l
