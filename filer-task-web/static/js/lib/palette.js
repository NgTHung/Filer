// Selection and matching rules for the command palette, separated from the
// overlay so keyboard behaviour can be tested without a DOM.

export function filterProjects(projects, query) {
  const needle = (query ?? "").trim().toLowerCase();
  if (needle === "") {
    return projects ?? [];
  }
  return (projects ?? []).filter((project) => project.name.toLowerCase().includes(needle));
}

// Wraps rather than clamps so holding an arrow key cycles the list. An index
// left past the end by a narrowing query is pulled back to the last row first,
// so the next arrow lands where the list actually ends rather than mid-list.
export function moveSelection(index, delta, length) {
  if (length <= 0) {
    return 0;
  }
  const current = Math.min(Math.max(index, 0), length - 1);
  return (((current + delta) % length) + length) % length;
}

export function isPaletteShortcut(event) {
  return (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k";
}
