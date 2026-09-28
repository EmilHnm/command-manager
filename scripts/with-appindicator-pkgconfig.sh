#!/bin/sh
# Tauri's bundler locates the tray library with pkg-config. The runtime .so is
# enough for that lookup when the matching -dev package is not installed.
if ! pkg-config --exists ayatana-appindicator3-0.1 \
  && ! pkg-config --exists appindicator3-0.1; then
  pcdir="src-tauri/target/pkgconfig"
  mkdir -p "$pcdir"
  write_pc() {
    name="$1"
    lib="$2"
    dir=$(dirname "$lib")
    cat > "$pcdir/$name.pc" <<EOF
Name: $name
Description: Installed AppIndicator runtime library
Version: 0.5.94
Libs: -L$dir -l$name
Cflags:
EOF
  }
  ayatana=$(ldconfig -p 2>/dev/null | awk '/libayatana-appindicator3\.so\.1 / { print $NF; exit }')
  legacy=$(ldconfig -p 2>/dev/null | awk '/libappindicator3\.so\.1 / { print $NF; exit }')
  if [ -n "$ayatana" ]; then
    write_pc ayatana-appindicator3-0.1 "$ayatana"
  elif [ -n "$legacy" ]; then
    write_pc appindicator3-0.1 "$legacy"
  fi
  if [ -d "$pcdir" ]; then
    PKG_CONFIG_PATH="$pcdir${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    export PKG_CONFIG_PATH
  fi
fi
exec "$@"
