import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import {
  activeFilterEntries,
  hasActiveFilters,
  rejectionFor,
  withoutField,
} from "../../static/js/lib/filters.js";

test("unset, empty and false filters are not active", () => {
  const filters = { status: "To Do", priority: undefined, domain: "", blocked: false, tag: "web" };

  assert.deepEqual(activeFilterEntries(filters), [
    ["status", "To Do"],
    ["tag", "web"],
  ]);
  assert.equal(hasActiveFilters(filters), true);
  assert.equal(hasActiveFilters({ blocked: false }), false);
  assert.equal(hasActiveFilters(undefined), false);
});

test("blocked only counts as applied when it is on", () => {
  assert.deepEqual(activeFilterEntries({ blocked: true }), [["blocked", true]]);
});

test("dropping a field leaves the other filters untouched", () => {
  const filters = { status: "To Do", tag: "web" };

  assert.deepEqual(withoutField(filters, "tag"), { status: "To Do", tag: undefined });
  assert.deepEqual(filters, { status: "To Do", tag: "web" });
});

test("a rejected tag names the field that carried the bad value", () => {
  const error = new ApiError(422, {
    error: "tag frontend is not in the catalog",
    code: "tag_rejected",
    context: { rejected_value: "frontend", allowed: ["backend", "tasks"] },
  });

  assert.deepEqual(rejectionFor(error, { tag: "frontend" }), {
    field: "tag",
    value: "frontend",
    message: "tag frontend is not in the catalog",
    candidates: [],
  });
});

test("an ambiguous parent reference carries its candidates", () => {
  const error = new ApiError(422, {
    error: "ambiguous reference WORK-001",
    code: "ambiguous_reference",
    context: { candidates: ["backend:WORK-001", "release:WORK-001"] },
  });

  const rejection = rejectionFor(error, { parent: "WORK-001" });

  assert.deepEqual(rejection.candidates, ["backend:WORK-001", "release:WORK-001"]);
});

test("a missing parent is rejected as a parent filter", () => {
  const error = new ApiError(404, {
    error: "task WORK-999 not found",
    code: "task_not_found",
    context: { reference: "WORK-999" },
  });

  assert.equal(rejectionFor(error, { parent: "WORK-999" }).field, "parent");
});

test("errors unrelated to a set filter are not rejections", () => {
  const notSet = new ApiError(422, { error: "rejected", code: "tag_rejected" });
  const otherCode = new ApiError(400, { error: "invalid sort_by: Title", code: "bad_request" });

  assert.equal(rejectionFor(notSet, { status: "To Do" }), null);
  assert.equal(rejectionFor(otherCode, { tag: "web" }), null);
  assert.equal(rejectionFor(new TypeError("offline"), { tag: "web" }), null);
});
