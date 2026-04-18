import socket
import sys

def send_logs():
    try:
        s = socket.create_connection(('127.0.0.1', 8765))
        logs = [
            "java.lang.NullPointerException: Cannot invoke \"String.length()\" because \"<local0>\" is null",
            "\tat EdgeCase.inner(EdgeCase.java:16)",
            "\tat EdgeCase.execute(EdgeCase.java:11)",
            "\tat EdgeCase.main(EdgeCase.java:4)"
        ]
        for line in logs:
            s.sendall((line + "\n").encode('utf-8'))
        s.close()
        print("Logs sent successfully")
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    send_logs()
