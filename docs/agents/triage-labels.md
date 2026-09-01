# Triage Labels

Filer stores triage roles as tags in the configured `triage-state` exclusive group.

| Canonical role | Filer tag | Meaning |
| --- | --- | --- |
| `needs-triage` | `needs-triage` | A maintainer needs to evaluate the task |
| `needs-info` | `needs-info` | More information is required |
| `ready-for-agent` | `ready-for-agent` | The task is specified for agent implementation |
| `ready-for-human` | `ready-for-human` | The task requires human implementation |
| `wontfix` | `wontfix` | The request will not be actioned |

The `triage-category` group contains `bug` and `enhancement`.

A triaged task carries exactly one category and one state. An untriaged task may carry neither. `taskroot` rejects multiple values from one group.

Set or clear roles through `taskroot tag set` and `taskroot tag clear`. Do not edit the full tag list to perform a triage transition.

Triage state and lifecycle status answer different questions. A `ready-for-agent` task is executable only when `taskroot ready --tag ready-for-agent` returns it.
