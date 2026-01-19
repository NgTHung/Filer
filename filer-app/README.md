# filer-app

Main application binary for the Filer file explorer.

## Overview

This crate is the entry point that combines `filer-core` with a GUI frontend.

## Running

```bash
cargo run -p filer-app
```

## Building

```bash
# Debug
cargo build -p filer-app

# Release
cargo build -p filer-app --release
```

## Features

```bash
# Enable all remote providers
cargo run -p filer-app --features "s3,webdav,ftp,fuse,k8s"

# Enable encryption
cargo run -p filer-app --features "encryption"
```
```
