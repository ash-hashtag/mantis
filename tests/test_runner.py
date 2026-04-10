#!/usr/bin/env python3
"""
Mantis compilation test runner.

Compiles every .ms example from examples/ and reports which ones
succeed and which ones fail, along with stderr output for failures.
"""

import os
import subprocess
import glob
import sys
import time


# ANSI color codes
class Color:
    GREEN = "\033[92m"
    RED = "\033[91m"
    YELLOW = "\033[93m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RESET = "\033[0m"


def compile_example(ms_file, project_root):
    """
    Attempt to compile a single .ms file using `cargo run -- <file>`.
    Returns (success: bool, stdout: str, stderr: str, duration: float).
    """
    start = time.time()
    try:
        result = subprocess.run(
            ["cargo", "run", "--quiet", "--", ms_file],
            capture_output=True,
            text=True,
            timeout=60,
            cwd=project_root,
        )
    except subprocess.TimeoutExpired:
        duration = time.time() - start
        return False, "", "TIMEOUT after 60s", duration
    except Exception as e:
        duration = time.time() - start
        return False, "", f"Failed to run cargo: {e}", duration

    duration = time.time() - start
    success = result.returncode == 0
    return success, result.stdout, result.stderr, duration


def main():
    # Determine project root (script lives in tests/)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)

    # Find all .ms examples
    example_dir = os.path.join(project_root, "examples")
    ms_files = sorted(glob.glob(os.path.join(example_dir, "*.ms")))

    if not ms_files:
        print(f"No .ms files found in {example_dir}")
        sys.exit(1)

    print(f"{Color.BOLD}Mantis Compilation Test Runner{Color.RESET}")
    print(f"Found {len(ms_files)} example(s) in examples/\n")
    print(f"{'─' * 60}")

    passed = []
    failed = []

    for ms_file in ms_files:
        name = os.path.basename(ms_file)
        sys.stdout.write(f"  Compiling {name:<25s} ... ")
        sys.stdout.flush()

        success, stdout, stderr, duration = compile_example(ms_file, project_root)

        if success:
            print(f"{Color.GREEN}OK{Color.RESET}  {Color.DIM}({duration:.1f}s){Color.RESET}")
            passed.append(name)
        else:
            print(f"{Color.RED}FAIL{Color.RESET}  {Color.DIM}({duration:.1f}s){Color.RESET}")
            failed.append((name, stderr.strip(), stdout.strip()))

    # ── Summary ──────────────────────────────────────────────────
    print(f"{'─' * 60}\n")
    total = len(ms_files)
    print(f"{Color.BOLD}Results: {len(passed)}/{total} passed, {len(failed)}/{total} failed{Color.RESET}\n")

    if passed:
        print(f"{Color.GREEN}Passed ({len(passed)}):{Color.RESET}")
        for name in passed:
            print(f"  ✓ {name}")
        print()

    if failed:
        print(f"{Color.RED}Failed ({len(failed)}):{Color.RESET}")
        for name, stderr, stdout in failed:
            print(f"  ✗ {name}")
            if stderr:
                for line in stderr.splitlines():
                    print(f"      {Color.DIM}{line}{Color.RESET}")
            if stdout:
                for line in stdout.splitlines():
                    print(f"      {Color.DIM}[stdout] {line}{Color.RESET}")
            print()

    sys.exit(0 if not failed else 1)


if __name__ == "__main__":
    main()
