import sys
import time

print("stdout: line 1 - starting up")
sys.stdout.flush()
time.sleep(0.1)
print("stdout: line 2 - loading config")
sys.stdout.flush()
time.sleep(0.1)
print("stdout: line 3 - server ready")
sys.stdout.flush()
print("stderr: ERROR database connection failed", file=sys.stderr)
sys.stderr.flush()
