// Everything the creation form derives before it posts. The slug and the path
// mirror new_task_path and slug in filer-task/src/lifecycle.rs, so the preview
// names the file the server will actually write; changing either there without
// changing this makes the preview lie.

import { ApiError } from "../api/client.js";

const ALPHANUMERIC = /^[0-9A-Za-z]$/;
const DIGITS = /^[0-9]+$/;

export function slugify(title) {
  let slug = "";
  for (const character of title) {
    if (ALPHANUMERIC.test(character)) {
      slug += character.toLowerCase();
    } else if (!slug.endsWith("-")) {
      slug += "-";
    }
  }
  return slug.replace(/^-+/, "").replace(/-+$/, "");
}

// Padding copies the widest existing id rather than a fixed width, so a project
// numbering past 999 keeps its own convention instead of being reset to three.
export function nextNumber(tasks, domain, prefix) {
  let highest = 0;
  let width = 3;
  for (const task of tasks ?? []) {
    if (task.domain !== domain || !task.id.startsWith(`${prefix}-`)) {
      continue;
    }
    const suffix = task.id.slice(prefix.length + 1);
    if (!DIGITS.test(suffix)) {
      continue;
    }
    const value = Number(suffix);
    if (value >= highest) {
      highest = value;
      width = suffix.length;
    }
  }
  return String(highest + 1).padStart(width, "0");
}

export function preview(domain, prefix, number, title) {
  if (!domain || !prefix || !number) {
    return null;
  }
  const id = `${prefix}-${number}`;
  return {
    qualifiedId: `${domain}:${id}`,
    path: `.tasks/${domain}/${id}-${slugify(title ?? "")}.md`,
  };
}

// The server already names the offending input on the error body, so the form
// only routes it; anything unattributed belongs above the form, not beside an
// arbitrary field.
export function fieldError(error) {
  if (!(error instanceof ApiError)) {
    return { field: null, message: error.message, allowed: [] };
  }
  const allowed = error.context && error.context.allowed;
  return {
    field: error.field ?? null,
    message: error.message,
    allowed: Array.isArray(allowed) ? allowed : [],
  };
}
