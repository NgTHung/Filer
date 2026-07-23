import assert from "node:assert/strict";
import { test } from "node:test";

import { SERVER_SORT_KEY, isServerSorted, sortRows } from "../../static/js/lib/sorting.js";

function row(id, extra = {}) {
  return {
    qualified_id: id,
    title: id,
    status: "To Do",
    priority: "High",
    type: "Feature",
    milestone: null,
    last_updated: null,
    ...extra,
  };
}

test("server sort keys match the sort_by values the list endpoint accepts", () => {
  assert.deepEqual(SERVER_SORT_KEY, { id: "Id", status: "Status", priority: "Priority" });
});

test("server-sorted columns keep the order the endpoint returned", () => {
  const rows = [row("core:CORE-002", { status: "Done" }), row("core:CORE-001")];

  assert.deepEqual(
    sortRows(rows, "status", "asc").map((task) => task.qualified_id),
    ["core:CORE-002", "core:CORE-001"],
  );
});

test("descending reverses the server order", () => {
  const rows = [row("core:CORE-002", { status: "Done" }), row("core:CORE-001")];

  assert.deepEqual(
    sortRows(rows, "status", "desc").map((task) => task.qualified_id),
    ["core:CORE-001", "core:CORE-002"],
  );
});

test("client-only columns sort locally in both directions", () => {
  const rows = [row("core:CORE-001", { title: "Routing" }), row("core:CORE-002", { title: "Caching" })];

  assert.deepEqual(
    sortRows(rows, "title", "asc").map((task) => task.title),
    ["Caching", "Routing"],
  );
  assert.deepEqual(
    sortRows(rows, "title", "desc").map((task) => task.title),
    ["Routing", "Caching"],
  );
});

test("missing milestone and updated values sort without throwing", () => {
  const rows = [row("core:CORE-001"), row("core:CORE-002", { milestone: "0.3.0" })];

  assert.deepEqual(
    sortRows(rows, "milestone", "asc").map((task) => task.qualified_id),
    ["core:CORE-001", "core:CORE-002"],
  );
});

test("sorting does not mutate the source rows", () => {
  const rows = [row("core:CORE-002"), row("core:CORE-001")];

  sortRows(rows, "id", "desc");

  assert.deepEqual(
    rows.map((task) => task.qualified_id),
    ["core:CORE-002", "core:CORE-001"],
  );
});

test("only columns with a query param are server sorted", () => {
  assert.equal(isServerSorted("priority"), true);
  assert.equal(isServerSorted("updated"), false);
});
