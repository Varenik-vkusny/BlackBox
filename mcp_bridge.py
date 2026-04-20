import sys
import subprocess
import os
import json
import datetime

LOG_FILE = r"C:\Users\user\Desktop\BlackBox\mcp_bridge.log"

def log(msg):
    with open(LOG_FILE, "a") as f:
        ts = datetime.datetime.now().isoformat()
        f.write(f"[{ts}] {msg}\n")

log("Bridge started")
log(f"Arguments: {sys.argv}")
log(f"Environment: {os.environ.get('PATH')[:100]}...")

cmd = [r"C:\Users\user\Desktop\BlackBox\target\debug\blackbox-daemon.exe", "--cwd", r"C:\Users\user\Desktop\BlackBox"]
log(f"Spawning: {cmd}")

try:
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=0 # Unbuffered
    )
except Exception as e:
    log(f"FAILED TO SPAWN: {e}")
    sys.exit(1)

log(f"Spawned with PID: {proc.pid}")

# Forwarding threads
import threading

def forward_stderr():
    for line in proc.stderr:
        log(f"RUST-STDERR: {line.strip()}")

threading.Thread(target=forward_stderr, daemon=True).start()

def forward_stdout():
    for line in proc.stdout:
        # log(f"RUST-STDOUT: {line.strip()}") # Don't log full JSON to keep bridge log clean
        sys.stdout.write(line)
        sys.stdout.flush()

threading.Thread(target=forward_stdout, daemon=True).start()

try:
    for line in sys.stdin:
        log(f"HOST-STDIN: {line.strip()}")
        proc.stdin.write(line)
        proc.stdin.flush()
except EOFError:
    log("HOST STDIN EOF")
except Exception as e:
    log(f"HOST STDIN ERROR: {e}")

log("Bridge exiting")
