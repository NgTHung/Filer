# Taskroot

Taskroot keeps project work in structured Markdown files under `.tasks/`. The
files stay readable, reviewable in Git, and available to both people and coding
agents without a hosted issue tracker.

Taskroot provides:

- project-local domains and task identities
- configurable prefixes, task types, and tags
- task dependencies, milestones, and acceptance criteria
- lifecycle commands for ready, active, blocked, and completed work
- human-readable and JSON output
- a Rust library for applications that need the same task contract

## Install

Install the command from crates.io:

```bash
cargo install taskroot --locked
```

## Start a task project

Run `init` at the root of any project:

```bash
cd /path/to/your-project
taskroot init --domain work --prefix WORK
taskroot validate
```

This creates `.tasks/config.json`. Domain directories are created when you add
their first task.

```bash
taskroot add \
  --id work:WORK-001 \
  --title "Add project health check" \
  --priority High \
  --type Feature \
  --criterion "The health check reports dependency failures."

taskroot ready
taskroot start work:WORK-001
taskroot criterion-toggle work:WORK-001 0
taskroot done work:WORK-001
```

Commands discover the nearest ancestor containing `.tasks/`. Use `--root` to
operate on another project explicitly:

```bash
taskroot list --root ../another-project --format json
```

## Use the library

Library callers open an explicit task project. Discovery remains optional so a
long-lived process can work with several projects safely.

```rust
use taskroot::{project::TaskProject, repo::discover_project_root};

let root = discover_project_root(std::env::current_dir()?)?;
let project = TaskProject::open(root)?;
println!("{}", project.root().display());
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [task tracking guide](https://github.com/NgTHung/Filer/blob/main/docs/task-tracking.md)
for the configuration format and command reference.

## Migrate from filer-task

Taskroot retains the `.tasks/` project format. Replace the old executable and
continue using the same task files:

```bash
cargo uninstall filer-task
cargo install taskroot --locked
taskroot validate
```

Rust consumers should rename the dependency and imports from `filer_task` to
`taskroot`.

## License

Taskroot is available under the MIT License.
