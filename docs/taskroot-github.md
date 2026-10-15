# Publish tasks to GitHub Projects

A GitHub Project can display Taskroot tasks without a second maintained backlog.
The publisher mirrors validated local tasks into draft items and keeps `.tasks/`
authoritative. Use a dedicated project because local values overwrite edits to
the managed draft content and fields.

## Prepare the destination

Install [GitHub CLI](https://cli.github.com/) and authenticate for Projects:

```bash
gh auth refresh --hostname github.com --scopes project
gh project list --owner NgTHung --format json
```

Choose a destination from the list or create an empty project when you are ready
to write to GitHub:

```bash
gh project create --owner NgTHung --title "Filer tasks" --format json
```

Record its numeric project number and its node ID, which starts with `PVT_`.
The publisher uses the node ID. The field commands below use the number.

Configure these fields, with names and option spelling exactly as shown:

| Field | Kind | Options |
|---|---|---|
| Status | Single select | To Do, In Progress, Blocked, Done, Deferred, Obsolete |
| Priority | Single select | High, Medium, Low |
| Taskroot ID | Text | |
| Domain | Text | |
| Taskroot Milestone | Text | |

Edit the built-in Status field in the project's settings. Rename its default
`Todo` option to `To Do` if needed and add the missing options. Keep existing
option identities when editing an existing project. GitHub explains field
management in [single-select fields](https://docs.github.com/en/issues/planning-and-tracking-with-projects/understanding-fields/about-single-select-fields).

Create the other fields when they are absent, replacing `NUMBER`:

```bash
gh project field-create NUMBER --owner NgTHung --name Priority --data-type SINGLE_SELECT --single-select-options "High,Medium,Low"
gh project field-create NUMBER --owner NgTHung --name "Taskroot ID" --data-type TEXT
gh project field-create NUMBER --owner NgTHung --name Domain --data-type TEXT
gh project field-create NUMBER --owner NgTHung --name "Taskroot Milestone" --data-type TEXT
```

Group a board view by Status and display Priority, Domain, and Taskroot Milestone
to browse the mirrored work. Native repository labels and milestones belong to
repository issues; the draft mirror uses the custom fields above.

## Preview and apply

Run from the checkout, replacing `PVT_PROJECT_ID`:

```bash
cargo run --locked -p taskroot -- github-sync --repository NgTHung/Filer --project PVT_PROJECT_ID
cargo run --locked -p taskroot -- github-sync --repository NgTHung/Filer --project PVT_PROJECT_ID --apply
```

The command publishes every validated task in the selected root, including
completed and deferred tasks. Use `--root PATH` for another Task project and
`--format json` for a structured plan. See the [publication rules](task-tracking.md#github-project-publication)
for ownership, retries, and retained items.

## Configure automation

The [publication workflow](../.github/workflows/taskroot-github.yml) is disabled for pushes until
`TASKROOT_GITHUB_SYNC_ENABLED` equals `true`. Its manual entry previews by
default and writes only when the `apply` input is selected. It runs from `main`
and serializes publication so two workflow runs cannot create the same draft.

Set repository variable `TASKROOT_GITHUB_PROJECT_ID` to the destination node ID.
Set repository secret `TASKROOT_GITHUB_TOKEN` to a credential with access to that
project. The built-in Actions token cannot access Projects. For a user project,
use a personal access token with the `project` scope; organization projects can
use a GitHub App installation token with organization Projects write permission.
See [GitHub's authentication guidance](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/automating-projects-using-actions).

After configuring the destination and credential, run a manual preview before
applying or enabling push publication. Push publication follows changes to
`.tasks/`, Taskroot, or the publication workflow. Credentials stay outside the
checkout. Avoid running the local publisher while the workflow is active.
