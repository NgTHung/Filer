import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError, toQueryString } from "../../static/js/api/client.js";

test("no query means no query string", () => {
  assert.equal(toQueryString(undefined), "");
  assert.equal(toQueryString({}), "");
});

test("unset values are dropped so a cleared filter stops being sent", () => {
  assert.equal(toQueryString({ status: undefined, tag: "", domain: null, priority: "High" }), "?priority=High");
});

test("filter values are encoded", () => {
  assert.equal(toQueryString({ status: "In Progress" }), "?status=In+Progress");
  assert.equal(toQueryString({ parent: "backend:WORK-001" }), "?parent=backend%3AWORK-001");
});

test("the blocked toggle and sort key travel as query params", () => {
  assert.equal(toQueryString({ blocked: true, sort_by: "Priority" }), "?blocked=true&sort_by=Priority");
});

test("an error body maps onto the structured fields the screens read", () => {
  const error = new ApiError(422, {
    error: "tag frontend is not in the catalog",
    code: "tag_rejected",
    field: "tags",
    context: { allowed: ["backend"] },
    issues: [],
  });

  assert.equal(error.status, 422);
  assert.equal(error.code, "tag_rejected");
  assert.equal(error.field, "tags");
  assert.deepEqual(error.context, { allowed: ["backend"] });
  assert.equal(error.message, "tag frontend is not in the catalog");
});

test("a body without an error message falls back to the status", () => {
  assert.equal(new ApiError(500, null).message, "request failed with status 500");
});
