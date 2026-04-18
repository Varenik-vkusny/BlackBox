#!/usr/bin/env bash
# BlackBox shell hook for Bash
#
# Sends each executed command line to the BlackBox daemon via TCP.
# Zero configuration: add one line to ~/.bashrc to activate.
#
# Usage:
#   echo 'source /path/to/blackbox.bash' >> ~/.bashrc
#
# Environment variables:
#   BLACKBOX_HOST  (default: 127.0.0.1)
#   BLACKBOX_PORT  (default: 8765)

BLACKBOX_HOST="${BLACKBOX_HOST:-127.0.0.1}"
BLACKBOX_PORT="${BLACKBOX_PORT:-8765}"

__blackbox_send() {
    local line="$1"
    # /dev/tcp is a bash builtin — no netcat/socat dependency needed
    (echo "$line" > /dev/tcp/"$BLACKBOX_HOST"/"$BLACKBOX_PORT") 2>/dev/null &
}

# DEBUG trap fires before each command execution
__blackbox_preexec() {
    # Skip empty commands and the trap itself
    [[ -z "$BASH_COMMAND" ]] && return
    [[ "$BASH_COMMAND" == __blackbox_* ]] && return
    __blackbox_send "$ $BASH_COMMAND"
}

trap '__blackbox_preexec' DEBUG
