def failing_function():
    print("Executing Python script to generate an error...")
    x = 1 / 0  # This will cause a ZeroDivisionError

def main():
    failing_function()

if __name__ == "__main__":
    main()
