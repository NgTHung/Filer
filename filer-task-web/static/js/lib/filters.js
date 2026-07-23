// Filter bookkeeping for the tasks screen: which applied filters are active,
// and which backend errors describe a bad filter value rather than a failed
// request. Shared by the screen and the filter menu so both agree on what
// counts as applied.

import { ApiError } from "../api/client.js";

// Codes the /tasks list endpoint returns when a filter value itself is invalid,
// mapped to the filter-menu field that caused them (see filer-task-web/src/error.rs).
export const REJECTABLE_CODES = {
  tag_rejected: "tag",
  ambiguous_reference: "parent",
  task_not_found: "parent",
};

export function activeFilterEntries(filters) {
  return Object.entries(filters ?? {}).filter(
    ([, value]) => value !== undefined && value !== null && value !== "" && value !== false,
  );
}

export function hasActiveFilters(filters) {
  return activeFilterEntries(filters).length > 0;
}

export function withoutField(filters, field) {
  return { ...filters, [field]: undefined };
}

// Describes the rejected filter when the error blames a value the query set,
// otherwise null so the caller surfaces it as a request failure.
export function rejectionFor(error, query) {
  if (!(error instanceof ApiError)) {
    return null;
  }
  const field = REJECTABLE_CODES[error.code];
  if (!field || query[field] === undefined) {
    return null;
  }
  const candidates = error.context && error.context.candidates;
  return {
    field,
    value: query[field],
    message: error.message,
    candidates: Array.isArray(candidates) ? candidates : [],
  };
}
