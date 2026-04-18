import json
import subprocess
import time
import os
import threading
import sys

def test_mcp():
    print("Starting blackbox-daemon with RUST_BACKTRACE=1...")
    env = os.environ.copy()
    env["RUST_BACKTRACE"] = "1"

    proc = subprocess.Popen(
        ["C:/Users/user/Desktop/BlackBox/target/debug/blackbox-daemon.exe", "--cwd", "C:/Users/user/Desktop/BlackBox"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
        env=env
    )

    # Stream stderr in background thread
    stderr_lines = []
    def read_stderr():
        for line in proc.stderr:
            line = line.rstrip()
            print(f"  [stderr] {line}", flush=True)
            stderr_lines.append(line)
    stderr_thread = threading.Thread(target=read_stderr, daemon=True)
    stderr_thread.start()

    def send(method, params=None, id=None):
        req = {"jsonrpc": "2.0", "method": method}
        if params is not None: req["params"] = params
        if id is not None: req["id"] = id
        msg = json.dumps(req)
        print(f"\nSEND: {msg}")
        proc.stdin.write(msg + "\n")
        proc.stdin.flush()

    def receive_line(timeout=3.0):
        """Read a line with a simple deadline."""
        import select as sel
        import io
        start = time.time()
        buf = ""
        # On Windows we can't use select on pipes, so just readline with a thread
        result = [None]
        def _read():
            result[0] = proc.stdout.readline()
        t = threading.Thread(target=_read, daemon=True)
        t.start()
        t.join(timeout=timeout)
        return result[0] or ""

    try:
        time.sleep(0.2)  # let daemon start

        # 1. Initialize
        send("initialize", {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "test", "version": "1.0"},
            "capabilities": {}
        }, id=1)
        line = receive_line()
        print(f"RECV: {line.strip()}")

        # 2. Initialized notification
        send("notifications/initialized")
        time.sleep(0.1)

        # 3. Tools list
        send("tools/list", id=2)
        line = receive_line()
        print(f"RECV: {line.strip()[:80]}...")

        # 4. Tool calls — test each tool
        tools_to_test = [
            ("get_snapshot", {}),
            ("get_terminal_buffer", {"lines": 10}),
            ("get_project_metadata", {}),
            ("get_compressed_errors", {"limit": 5}),
            ("get_contextual_diff", {}),
            ("get_container_logs", {}),
            ("get_postmortem", {"minutes": 1}),
            ("get_correlated_errors", {"window_secs": 5, "limit": 5}),
        ]

        for i, (tool_name, args) in enumerate(tools_to_test, start=3):
            print(f"\n--- Testing: {tool_name} ---")
            send("tools/call", {"name": tool_name, "arguments": args}, id=100 + i)
            line = receive_line(timeout=5.0)
            if line:
                parsed = json.loads(line)
                if "error" in parsed:
                    print(f"  ERROR: {parsed['error']}")
                else:
                    content = parsed.get("result", {}).get("content", [])
                    text = content[0]["text"] if content else "<no content>"
                    print(f"  OK: {text[:120]}")
            else:
                print(f"  TIMEOUT or PIPE CLOSED for {tool_name}!")
                break

    except Exception as e:
        print(f"\nTest EXCEPTION: {e}")
        import traceback
        traceback.print_exc()
    finally:
        print("\n--- Terminating daemon ---")
        proc.stdin.close()
        time.sleep(0.5)
        proc.terminate()
        proc.wait(timeout=3)
        stderr_thread.join(timeout=1)

if __name__ == "__main__":
    test_mcp()
