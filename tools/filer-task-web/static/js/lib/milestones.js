// Presentation shape for one milestone aggregation. tasks_by_status arrives as
// a Rust BTreeMap, so its keys are alphabetical and empty statuses are absent;
// both need fixing before a reader can scan a milestone top to bottom.

// Deliberately not shared with FilterMenu's STATUS_OPTIONS: that list is a query
// vocabulary the list endpoint accepts, this is a display order, and the two are
// free to diverge without either module needing to know about the other.
const STATUS_ORDER = ["To Do", "In Progress", "Blocked", "Done", "Deferred", "Obsolete"];

// The milestone names a form may offer. `current` is folded in so a task
// already carrying a milestone the aggregation no longer lists keeps it as a
// selectable option instead of silently resetting to None.
export function milestoneOptions(aggregations, current) {
  const values = (aggregations ?? []).map((entry) => entry.milestone.milestone).filter(Boolean);
  return current && !values.includes(current) ? [current, ...values] : values;
}

export function progressPercent(done, total) {
  if (!total) {
    return 0;
  }
  return Math.round((done / total) * 100);
}

export function statusGroups(tasksByStatus) {
  const entries = Object.entries(tasksByStatus ?? {}).filter(([, tasks]) => tasks.length > 0);
  return entries.sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));
}

// An unconfigured status still renders, after the lifecycle it does not belong to.
function rank(status) {
  const index = STATUS_ORDER.indexOf(status);
  return index === -1 ? STATUS_ORDER.length : index;
}
