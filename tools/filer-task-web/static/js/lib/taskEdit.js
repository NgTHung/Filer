// Everything the detail drawer derives to edit a task's own fields. Kept out of
// the components so the draft, the reference form, and the change diff stay
// unit-testable without a DOM.

import { splitList } from "./text.js";

// The three fields the server clears with null; tags and depends_on clear with
// an empty array instead, because null on those is a deserialize rejection.
const NULLABLE_FIELDS = ["risk", "impact", "milestone", "parent"];

// Every TaskView requalifies stored relationships to `domain:ID` while the
// markdown keeps the bare form, so echoing the view back would rewrite every
// in-domain reference in the file for no reason.
export function localizeRef(reference, domain) {
  if (!reference) {
    return "";
  }
  const prefix = `${domain}:`;
  return reference.startsWith(prefix) ? reference.slice(prefix.length) : reference;
}

// Optional metadata is omitted from the response rather than sent as null, so
// absence and "cleared" both have to arrive here as the empty input.
export function editDraft(detail) {
  const task = detail.task;
  const domain = task.domain;
  return {
    title: task.title ?? "",
    summary: sectionContent(detail.sections, "Summary"),
    risk: task.risk ?? "",
    impact: task.impact ?? "",
    tagsText: (task.tags ?? []).join(", "),
    milestone: task.milestone ?? "",
    parent: localizeRef(task.parent, domain),
    dependsText: (task.depends_on ?? []).map((entry) => localizeRef(entry, domain)).join(", "),
  };
}

// Only changed fields are sent: an absent key means keep, so a patch built this
// way can never overwrite a field the user did not touch.
export function editPatch(draft, baseline) {
  const patch = {};
  for (const field of ["title", "summary"]) {
    const value = draft[field].trim();
    if (value !== baseline[field].trim()) {
      patch[field] = value;
    }
  }
  for (const field of NULLABLE_FIELDS) {
    const value = draft[field].trim();
    if (value !== baseline[field].trim()) {
      patch[field] = value === "" ? null : value;
    }
  }
  addListChange(patch, "tags", draft.tagsText, baseline.tagsText);
  addListChange(patch, "depends_on", draft.dependsText, baseline.dependsText);
  return patch;
}

function addListChange(patch, field, text, baselineText) {
  const value = splitList(text);
  if (!sameList(value, splitList(baselineText))) {
    patch[field] = value;
  }
}

// Order is part of the stored value, so a reorder is a real edit.
function sameList(left, right) {
  return left.length === right.length && left.every((entry, index) => entry === right[index]);
}

function sectionContent(sections, heading) {
  const section = (sections ?? []).find((entry) => entry.heading === heading);
  return section ? section.content : "";
}
