Source docs/tapes/common.tape
Output docs/wiki/assets/hero.gif
Set Framerate 15

Hide
Type "export CLIDRAW_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d) PATH=$PWD/target/release:$PATH" Enter
Type "clidraw new flow >/dev/null && clidraw edit flow < docs/tapes/seed/flow.json >/dev/null && clear" Enter
Show
Type "clidraw flow" Enter
Sleep 1s
Type "z"
Sleep 1500ms
Type "/"
Type "Deploy"
Enter
Sleep 1500ms
Ctrl+p
Sleep 500ms
Type "Fit"
Sleep 1s
Enter
Sleep 1500ms
Type "?"
Sleep 2s
Escape
Sleep 500ms
Ctrl+c
