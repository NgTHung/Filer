import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import { fieldError } from "../../static/js/lib/rejection.js";

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

test("a rejected edit is routed to the relationship field the server named", () => {
  const cycle = new ApiError(422, {
    error: "dependency cycle detected: core:CORE-001 -> core:CORE-002",
    code: "validation_error",
    field: "depends_on",
    context: {},
  });

  assert.deepEqual(fieldError(cycle), {
    field: "depends_on",
    message: "dependency cycle detected: core:CORE-001 -> core:CORE-002",
    allowed: [],
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
