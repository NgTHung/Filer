// Everything the detail drawer derives from a ContextView before it renders or
// writes. Kept out of the components so the refusal, conflict, and graph rules
// stay unit-testable without a DOM.

import { ApiError } from "../api/client.js";

// The three transitions the server requires a reason for (see src/dto.rs
// ReasonRequest); start and done take no body.
export const REASON_ACTIONS = ["block", "defer", "obsolete"];

// Statuses 412 and 428 both mean the criterion the drawer targeted is no longer
// the one on disk, so the only safe response is to reload rather than retry.
const CONFLICT_STATUSES = [412, 428];

// The drawer refuses Done itself instead of letting the server reject it: the
// server's message names the heading, not which items are still open.
export function doneRefusal(criteria) {
  const indexes = (criteria ?? []).reduce(
    (open, item, index) => (item.checked ? open : [...open, index]),
    [],
  );
  return indexes.length === 0 ? null : { count: indexes.length, indexes };
}

export function isConflict(error) {
  return error instanceof ApiError && CONFLICT_STATUSES.includes(error.status);
}

// Ancestors are plain TaskViews and the other three are RelatedTaskViews, so
// both shapes collapse to the same chip here.
export function relationGroups(context) {
  const groups = [
    { heading: "Parent chain", chips: (context.ancestors ?? []).map(chip) },
    { heading: "Children", chips: (context.children ?? []).map((related) => chip(related.task)) },
    {
      heading: "Dependencies",
      chips: (context.dependencies ?? []).map((related) => chip(related.task)),
    },
    {
      heading: "Dependents",
      chips: (context.dependents ?? []).map((related) => chip(related.task)),
    },
  ];
  return groups.filter((group) => group.chips.length > 0);
}

// Readiness blockers always include the task's own non-To Do status, which is
// not something it is blocked by; only blockers naming another task are.
export function blockedByChain(context) {
  const task = context.detail.task;
  if (task.status !== "Blocked") {
    return [];
  }
  return (context.readiness?.blockers ?? [])
    .filter((blocker) => blocker.task_id && blocker.task_id !== task.qualified_id)
    .map((blocker) => ({
      qualified_id: blocker.task_id,
      kind: blocker.kind,
      status: blocker.status ?? null,
    }));
}

function chip(task) {
  return { qualified_id: task.qualified_id, title: task.title, status: task.status };
}
