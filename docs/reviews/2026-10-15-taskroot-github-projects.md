# Taskroot and GitHub Projects, 2026-10-15

People using Taskroot may want a GitHub board without maintaining a second copy
of every task by hand. GitHub Projects exposes the API operations needed for a
one-way mirror and a controlled return path. This research proposes starting
with publication from Taskroot, then importing selected edits through review.

## Choose the GitHub item

| Representation | Fit | Constraint |
|---|---|---|
| Project draft issue | A board backed by Taskroot tasks | Exists within one project; no native repository, labels, or milestone. |
| Repository issue added to a project | GitHub discussions and repository issue workflows | Introduces another editable task description and repository issue lifecycle. |

Draft issues support title, body, assignees, and project custom fields. GitHub
requires conversion to an issue before setting repository, labels, or native
milestones. A draft mirror could instead expose Taskroot milestone and tags as
custom fields. See [adding project items](https://docs.github.com/en/issues/planning-and-tracking-with-projects/managing-items-in-your-project/adding-items-to-your-project#creating-draft-issues).

## API coverage

GraphQL provides `addProjectV2DraftIssue`, `addProjectV2ItemById`, field discovery,
item queries, and `updateProjectV2ItemFieldValue`. Adding an existing issue twice
returns its existing project item ID. Creation and subsequent field updates use
separate calls. Issue-owned assignees, labels, and milestone require issue APIs
rather than project field updates. See [Projects API examples](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects).

`updateProjectV2DraftIssue` edits title and body using `draftIssueId`, which is
the draft content ID, distinct from the project item ID. Project field updates
support single-select, multi-select, text, number, date, and iteration values.
The schema also provides field creation, value clearing, and item archiving.
See the [Projects GraphQL schema](https://docs.github.com/en/graphql/reference/projects#updateprojectv2draftissue).

GitHub also documents [REST Projects endpoints](https://docs.github.com/en/rest/projects/projects).
GraphQL is a viable proposed adapter interface, not the only available API.

## Authenticate for the project owner

Classic personal access tokens use `read:project` for reads or `project` for
writes. GitHub CLI accepts those scopes during authentication. Additional
repository permissions may apply to mutations involving repository content.
See [API authentication](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-api-to-manage-projects#authentication).

The Actions `GITHUB_TOKEN` cannot access Projects. GitHub recommends an App for
organization projects and a personal access token for user projects. An App
needs organization Projects read/write permissions; repository Projects
permission alone is insufficient. Creating or changing repository issues also
needs the corresponding repository permissions. See [Projects in Actions](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/automating-projects-using-actions).

## Account for delivery and limits

The `projects_v2_item` webhook covers organization projects and requires at least
organization Projects read access for an App. GitHub documents these project
webhooks as public preview. Personal-project synchronization therefore needs a
polling or explicit pull path rather than assuming webhook delivery. See
[project item webhooks](https://docs.github.com/en/webhooks/webhook-events-and-payloads#projects_v2_item).

GitHub applies primary GraphQL point limits and secondary limits. Its general
content-generation limits are 80 requests per minute and 500 per hour; some
endpoints impose lower limits. GraphQL can return HTTP 200 with errors. An
adapter must inspect errors, paginate results, honor rate-limit responses, and
resume partial operations. See [GraphQL limits](https://docs.github.com/en/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api).

## Proposed stages

Taskroot already exposes qualified identities and task details through its
[JSON commands](../task-tracking.md#agent-workflow). Its
[editing library](../../tools/taskroot/src/lifecycle/edit.rs) validates partial
edits, but its patch type does not expose priority or status. A priority import
needs an additional validated operation. Existing
[lifecycle operations](../../tools/taskroot/src/lifecycle.rs) reject completion
when criteria remain unchecked, so a board move to Done cannot bypass that
requirement. No GitHub adapter exists in the inspected CLI.

1. Publish a selected Task project scope as draft issues. Keep Taskroot as the
   authority. Map qualified task ID, title, summary, status, priority, domain,
   and milestone to a documented set of board fields. Begin with an explicit
   preview and apply command; automate publication after merges if useful.
   Recover interrupted publication using a stable task marker in the initial
   draft body and a Taskroot ID text field. Retain GitHub node IDs and the last
   published values. Reconcile before retrying ambiguous creation responses,
   because the documentation does not promise idempotent draft creation.
   Report duplicate markers and inaccessible items instead of guessing. Include
   a stable source-project identity when one board contains several Task projects.
2. Import a small allowlist of GitHub edits into a branch and pull request.
   Compare local, remote, and last-synchronized values per field. Report
   conflicting edits explicitly. Apply lifecycle changes through Taskroot's
   validation rather than writing frontmatter directly, and suppress repeated
   export/import of the same change. Hold conflicting or pending inbound fields
   out of publication until review resolves them.

Native issue publication can be a separate stage when repository issue
discussions are needed. Dependency graphs, parent relationships, and task
acceptance criteria need explicit representations and should not be inferred
from board column names. Deletion, archival, and draft-to-issue conversion also
need policies before an unattended two-way process can run.

This proposal does not select implementation work or alter the active scope in
[task tracking](../task-tracking.md). The deferred
[external read-only publication task](../../.tasks/web/WEB-022-publish-task-progression-to-an-external-read-only-service.md)
has its own web contract; a CLI integration needs its own scoped decision.
