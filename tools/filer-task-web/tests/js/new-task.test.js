import assert from "node:assert/strict";
import { test } from "node:test";

import { nextNumber, parseTags } from "../../static/js/lib/newTask.js";

function task(domain, id) {
  return { domain, id, qualified_id: `${domain}:${id}`, title: id, status: "To Do" };
}

test("the next number is one past the highest id sharing the domain and prefix", () => {
  const tasks = [task("web", "WEB-001"), task("web", "WEB-021"), task("web", "WEB-007")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "022");
});

test("other domains and other prefixes do not raise the number", () => {
  const tasks = [task("core", "WEB-900"), task("web", "UTILS-500"), task("web", "WEB-003")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "004");
});

test("an unused domain and prefix pair starts at 001", () => {
  assert.equal(nextNumber([task("web", "WEB-004")], "core", "UTILS"), "001");
  assert.equal(nextNumber([], "web", "WEB"), "001");
});

test("padding follows the width of the highest existing id", () => {
  assert.equal(nextNumber([task("web", "WEB-0007")], "web", "WEB"), "0008");
  assert.equal(nextNumber([task("web", "WEB-9")], "web", "WEB"), "10");
});

test("ids whose suffix is not a plain number are ignored", () => {
  const tasks = [task("web", "WEB-A12"), task("web", "WEB-1-2"), task("web", "WEB-002")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "003");
});

test("comma-separated tags are split and trimmed", () => {
  assert.deepEqual(parseTags("web, tasks ,  ui"), ["web", "tasks", "ui"]);
});

test("empty segments from a trailing or doubled comma are dropped", () => {
  assert.deepEqual(parseTags("web,"), ["web"]);
  assert.deepEqual(parseTags("web,,tasks"), ["web", "tasks"]);
});

test("a string of only separators yields an empty array", () => {
  assert.deepEqual(parseTags(",,,"), []);
});

test("the empty string yields an empty array", () => {
  assert.deepEqual(parseTags(""), []);
});

test("surrounding whitespace on each tag is trimmed", () => {
  assert.deepEqual(parseTags("  web  ,  tasks-ui  "), ["web", "tasks-ui"]);
});
