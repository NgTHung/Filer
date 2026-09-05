# Feature validation

Run the feature matrix from any directory with Python 3.11 or newer:

```bash
python3 filer-core/tests/check_features.py
```

Use the script's absolute path when you run it outside the repository root.
The script reads filer-core/Cargo.toml, so new feature names enter the matrix
automatically. It checks minimal and default builds, every non-default feature
with defaults disabled, defaults with preview-code or preview, and all features.

Each configuration runs Cargo check for all targets, test compilation for all
targets, Clippy with warnings denied, library and integration tests, and doctests.
Benchmark targets compile but do not run. Tests marked ignored retain their
existing opt-in behavior.

Install a Rust toolchain with the Clippy component and the platform's native
build tools. The metadata-archive-rar configuration compiles bundled unrar C++
code, so it needs a C++ compiler and linker. Other archive dependencies also
build native code. On Linux, provide a C/C++ toolchain, make, and pkg-config.
The verified toolchain and platform are recorded in CORE-043's validation
evidence; they are evidence of one run, not a new minimum supported version.

You can inspect or narrow the matrix:

```bash
python3 filer-core/tests/check_features.py --list
python3 filer-core/tests/check_features.py --case minimal --phase check
python3 filer-core/tests/check_features.py --case preview-code --phase test --phase clippy
```

The script prints each command and its result. Logs go to target/feature-matrix
unless you provide --log-dir. It continues after a failed command to expose
independent failures, then exits with a nonzero status if any command failed.
Use that exit status when you call it from CI.

The initial minimal configuration failed because ZIP code and fixtures were
compiled without their optional dependency. Keep dependency imports, parsing
helpers, and format-specific tests behind the same feature gates. Public archive
entry points retain structured unsupported errors when archive support is off.
