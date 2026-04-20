import sys
import time

def deep_nested_error(level):
    if level <= 0:
        print("Reached depth, about to crash...")
        raise ValueError(f"CRITICAL REGRESSION! Data leak detected at level {level}!")
    deep_nested_error(level - 1)

def run_simulation(name):
    print(f"Starting simulation: {name}")
    time.sleep(0.5)
    try:
        deep_nested_error(3)
    except Exception as e:
        print(f"Caught exception in simulation {name}: {e}", file=sys.stderr)
        raise

if __name__ == "__main__":
    run_simulation("Advanced MCP Test")
