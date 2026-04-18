#!/usr/bin/env zsh
# BlackBox shell hook for Zsh
#
# Uses the standard preexec hook (built into zsh) to capture commands.
#
# Usage:
#   echo 'source /path/to/blackbox.zsh' >> ~/.zshrc

BLACKBOX_HOST="${BLACKBOX_HOST:-127.0.0.1}"
BLACKBOX_PORT="${BLACKBOX_PORT:-8765}"

__blackbox_send() {
    (echo "$1" > /dev/tcp/"$BLACKBOX_HOST"/"$BLACKBOX_PORT") 2>/dev/null &
}

# preexec is called by zsh just before a command is executed.
# $1 is the full command line as typed by the user.
preexec() {
    __blackbox_send "$ $1"
}
