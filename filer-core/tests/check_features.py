#!/usr/bin/env python3
"""Check filer-core feature isolation using only Python 3.11+ and Cargo."""

import argparse
from pathlib import Path
import shlex
import subprocess
import tomllib


def main():
    root = Path(__file__).resolve().parents[2]
    manifest = tomllib.loads((root / "filer-core/Cargo.toml").read_text())
    matrix = {
        "minimal": ["--no-default-features"],
        "default": [],
        **{
            feature: ["--no-default-features", "--features", feature]
            for feature in manifest["features"]
            if feature != "default"
        },
        "default-preview-code": ["--features", "preview-code"],
        "default-preview": ["--features", "preview"],
        "all": ["--all-features"],
    }
    phases = {
        "check": ["check", "--all-targets"],
        "compile-tests": ["test", "--all-targets", "--no-run"],
        "clippy": ["clippy", "--all-targets"],
        "test": ["test", "--lib", "--tests"],
        "doc": ["test", "--doc"],
    }
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", action="append", choices=matrix)
    parser.add_argument("--phase", action="append", choices=phases)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--log-dir", type=Path, default=root / "target/feature-matrix")
    args = parser.parse_args()
    selected = args.case or matrix
    if args.list:
        for case in selected:
            print(f"{case}: {shlex.join(matrix[case])}")
        return 0

    args.log_dir.mkdir(parents=True, exist_ok=True)
    failures = []
    for case in selected:
        for phase in args.phase or phases:
            command = ["cargo", *phases[phase], "-p", "filer-core", *matrix[case]]
            if phase == "clippy":
                command.extend(["--", "-D", "warnings"])
            log = args.log_dir / f"{case}-{phase}.log"
            print(f"{case}/{phase}: {shlex.join(command)}", flush=True)
            with log.open("w") as output:
                result = subprocess.run(
                    command, cwd=root, stdout=output, stderr=subprocess.STDOUT
                )
            status = "PASS" if result.returncode == 0 else "FAIL"
            print(f"{status}: {log}", flush=True)
            if result.returncode:
                failures.append(f"{case}/{phase}")
    if failures:
        print("Failed: " + ", ".join(failures))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
