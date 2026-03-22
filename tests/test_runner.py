import os
import subprocess
import glob
import sys

def run_test(ms_file):
    print(f"Running test for: {ms_file}")
    
    # Run the compiler with --run flag
    # cargo run --quiet -- <file> --run
    try:
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--", ms_file, "--run"],
            capture_output=True,
            text=True,
            cwd=os.getcwd()
        )
    except Exception as e:
        print(f"Error running cargo: {e}")
        return False, "Execution failed"

    # Identify matching .out file in tests/expected/
    base_name = os.path.basename(ms_file)
    test_name = os.path.splitext(base_name)[0]
    expected_out_file = os.path.join("tests", "expected", f"{test_name}.out")
    
    actual_stdout = result.stdout
    actual_exit_code = result.returncode

    if os.path.exists(expected_out_file):
        with open(expected_out_file, 'r') as f:
            expected_stdout = f.read()
        
        if actual_stdout.strip() == expected_stdout.strip():
            print(f"PASS: {test_name} (Stdout matches)")
            return True, None
        else:
            print(f"FAIL: {test_name} (Stdout mismatch)")
            print("--- Expected output ---")
            print(expected_stdout)
            print("--- Actual output ---")
            print(actual_stdout)
            print("--- Error output ---")
            print(result.stderr)
            return False, "Stdout mismatch"
    else:
        # No expected output file, just check exit code
        if actual_exit_code == 0:
            print(f"PASS: {test_name} (Success exit code)")
            return True, None
        else:
            print(f"FAIL: {test_name} (Exit code {actual_exit_code})")
            print("--- Error output ---")
            print(result.stderr)
            return False, "Non-zero exit code"

def main():
    ms_files = glob.glob(os.path.join("mantis", "src", "*.ms"))
    if not ms_files:
        # Fallback if the path is different
        ms_files = glob.glob(os.path.join("src", "*.ms"))
    
    if not ms_files:
        print("No .ms files found in mantis/src/")
        sys.exit(1)

    all_passed = True
    for ms_file in sorted(ms_files):
        passed, error = run_test(ms_file)
        if not passed:
            all_passed = False
    
    if all_passed:
        print("\nAll tests passed!")
        sys.exit(0)
    else:
        print("\nSome tests failed.")
        sys.exit(1)

if __name__ == "__main__":
    main()
