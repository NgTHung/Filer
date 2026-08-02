// Everything the creation form derives before it posts.

import { splitList } from "./text.js";

const DIGITS = /^[0-9]+$/;

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

export function parseTags(value) {
  return splitList(value);
}
