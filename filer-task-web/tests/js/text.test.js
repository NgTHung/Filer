import assert from "node:assert/strict";
import { test } from "node:test";

import { splitList } from "../../static/js/lib/text.js";

test("a comma-separated string is split and trimmed", () => {
  assert.deepEqual(splitList("web, tasks ,  ui"), ["web", "tasks", "ui"]);
});

test("empty segments from a trailing or doubled comma are dropped", () => {
  assert.deepEqual(splitList("web,"), ["web"]);
  assert.deepEqual(splitList("web,,tasks"), ["web", "tasks"]);
});

test("a string of only separators yields an empty array", () => {
  assert.deepEqual(splitList(",,,"), []);
});

test("the empty string yields an empty array", () => {
  assert.deepEqual(splitList(""), []);
});

test("surrounding whitespace on each entry is trimmed", () => {
  assert.deepEqual(splitList("  web  ,  tasks-ui  "), ["web", "tasks-ui"]);
});
