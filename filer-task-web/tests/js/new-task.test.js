import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import { fieldError, nextNumber, preview, slugify } from "../../static/js/lib/newTask.js";

function task(domain, id) {
  return { domain, id, qualified_id: `${domain}:${id}`, title: id, status: "To Do" };
}

test("the slug lowercases alphanumerics and collapses everything else to one dash", () => {
  assert.equal(slugify("Build the Milestones screen"), "build-the-milestones-screen");
  assert.equal(slugify("Serve ready-work  &  milestone endpoints"), "serve-ready-work-milestone-endpoints");
  assert.equal(slugify("Filer 2.0: what's next?"), "filer-2-0-what-s-next");
});

test("leading and trailing separators are trimmed off the slug", () => {
  assert.equal(slugify("  --Trailing punctuation!!  "), "trailing-punctuation");
  assert.equal(slugify("!!!"), "");
});

test("non-ascii characters become separators, matching the server", () => {
  assert.equal(slugify("Café résumé"), "caf-r-sum");
});

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

test("the preview shows the qualified id and the path the server will write", () => {
  assert.deepEqual(preview("web", "WEB", "030", "Build the Milestones screen"), {
    qualifiedId: "web:WEB-030",
    path: ".tasks/web/WEB-030-build-the-milestones-screen.md",
  });
});

test("a title with no slug-able characters still previews a valid path", () => {
  assert.deepEqual(preview("web", "WEB", "030", ""), {
    qualifiedId: "web:WEB-030",
    path: ".tasks/web/WEB-030-.md",
  });
});

test("an incomplete form previews nothing rather than a wrong path", () => {
  assert.equal(preview("", "WEB", "030", "Title"), null);
  assert.equal(preview("web", "", "030", "Title"), null);
  assert.equal(preview("web", "WEB", "", "Title"), null);
});

test("a rejection is routed to the field the server named", () => {
  const duplicate = new ApiError(422, {
    error: "task core:CORE-007 already exists",
    code: "id_exists",
    field: "number",
    context: { task: "core:CORE-007" },
  });

  assert.deepEqual(fieldError(duplicate), {
    field: "number",
    message: "task core:CORE-007 already exists",
    allowed: [],
  });
});

test("a rejected tag carries the catalog so the form can offer it", () => {
  const rejected = new ApiError(422, {
    error: "tag rejected is not in the catalog",
    code: "tag_rejected",
    field: "tags",
    context: { rejected_value: "rejected", allowed: ["web", "tasks"] },
  });

  assert.deepEqual(fieldError(rejected), {
    field: "tags",
    message: "tag rejected is not in the catalog",
    allowed: ["web", "tasks"],
  });
});

test("an error naming no field falls back to a form-level message", () => {
  const broken = new ApiError(422, { error: "project filer failed validation", project: "filer" });
  const offline = new TypeError("Failed to fetch");

  assert.deepEqual(fieldError(broken), {
    field: null,
    message: "project filer failed validation",
    allowed: [],
  });
  assert.deepEqual(fieldError(offline), { field: null, message: "Failed to fetch", allowed: [] });
});
