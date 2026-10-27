Source docs/tapes/common.tape

Hide
Type "export CLIDRAW_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d) PATH=$PWD/target/release:$PATH" Enter
Type "clidraw new flow >/dev/null && clidraw edit flow < docs/tapes/seed/flow.json >/dev/null && clear" Enter
Show
Type "clidraw flow" Enter
Sleep 1s
Type "z"
Sleep 1s
Type "?"
Sleep 1s
Screenshot docs/wiki/assets/shortcuts.png
Ctrl+c
