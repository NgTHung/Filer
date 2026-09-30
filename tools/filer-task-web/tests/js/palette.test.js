import assert from "node:assert/strict";
import { test } from "node:test";

import {
  filterProjects,
  isPaletteShortcut,
  moveSelection,
  paletteRows,
} from "../../static/js/lib/palette.js";

const PROJECTS = [
  { name: "filer", broken: false },
  { name: "Filer-Docs", broken: false },
  { name: "sandbox", broken: true },
];

test("an empty query keeps every project in registry order", () => {
  assert.deepEqual(filterProjects(PROJECTS, ""), PROJECTS);
  assert.deepEqual(filterProjects(PROJECTS, "   "), PROJECTS);
});

test("matching ignores case and matches anywhere in the name", () => {
  assert.deepEqual(
    filterProjects(PROJECTS, "FILER").map((project) => project.name),
    ["filer", "Filer-Docs"],
  );
  assert.deepEqual(
    filterProjects(PROJECTS, "box").map((project) => project.name),
    ["sandbox"],
  );
});

test("a query matching nothing yields an empty list", () => {
  assert.deepEqual(filterProjects(PROJECTS, "missing"), []);
});

test("selection wraps at both ends", () => {
  assert.equal(moveSelection(0, 1, 3), 1);
  assert.equal(moveSelection(2, 1, 3), 0);
  assert.equal(moveSelection(0, -1, 3), 2);
});

test("selection stays at zero when there is nothing to select", () => {
  assert.equal(moveSelection(0, 1, 0), 0);
  assert.equal(moveSelection(4, -1, 0), 0);
});

test("a selection past the end of a shrunken list comes back in range", () => {
  assert.equal(moveSelection(9, 1, 3), 0);
});

test("rows are the matching projects while the query still matches something", () => {
  assert.deepEqual(paletteRows(PROJECTS, "box"), [{ kind: "project", project: PROJECTS[2] }]);
});

test("a query matching nothing offers creating a project under that name", () => {
  assert.deepEqual(paletteRows(PROJECTS, "new-thing"), [{ kind: "create", name: "new-thing" }]);
});

test("the create row uses the trimmed query as the proposed name", () => {
  assert.deepEqual(paletteRows(PROJECTS, "  new-thing  "), [{ kind: "create", name: "new-thing" }]);
});

test("an empty query never offers a create row, even with no projects registered", () => {
  assert.deepEqual(paletteRows([], ""), []);
  assert.deepEqual(paletteRows([], "   "), []);
});

test("the shortcut is cmd-K or ctrl-K and nothing else", () => {
  assert.equal(isPaletteShortcut({ key: "k", metaKey: true, ctrlKey: false }), true);
  assert.equal(isPaletteShortcut({ key: "K", metaKey: false, ctrlKey: true }), true);
  assert.equal(isPaletteShortcut({ key: "k", metaKey: false, ctrlKey: false }), false);
  assert.equal(isPaletteShortcut({ key: "j", metaKey: true, ctrlKey: false }), false);
});
