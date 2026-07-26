import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import {
  REASON_ACTIONS,
  blockedByChain,
  doneRefusal,
  isConflict,
  relationGroups,
} from "../../static/js/lib/taskDetail.js";

function related(id, title, status) {
  return { task: { qualified_id: id, title, status }, readiness: { ready: false, blockers: [] } };
}

function context(overrides) {
  return {
    detail: {
      task: { qualified_id: "web:WEB-008", title: "Target task", status: "To Do" },
      criteria: [],
    },
    readiness: { ready: true, blockers: [] },
    parent: null,
    ancestors: [],
    children: [],
    dependencies: [],
    dependents: [],
    ...overrides,
  };
}

test("a refusal counts and locates every unchecked criterion", () => {
  const criteria = [{ checked: true }, { checked: false }, { checked: true }, { checked: false }];

  assert.deepEqual(doneRefusal(criteria), { count: 2, indexes: [1, 3] });
});

test("a fully checked list refuses nothing", () => {
  assert.equal(doneRefusal([{ checked: true }, { checked: true }]), null);
});

test("a task with no criteria refuses nothing", () => {
  assert.equal(doneRefusal([]), null);
  assert.equal(doneRefusal(undefined), null);
});

test("only a stale hash or a missing precondition counts as a conflict", () => {
  assert.equal(isConflict(new ApiError(412, { error: "criterion changed" })), true);
  assert.equal(isConflict(new ApiError(428, { error: "If-Match is required" })), true);
  assert.equal(isConflict(new ApiError(422, { error: "project failed validation" })), false);
  assert.equal(isConflict(new TypeError("Failed to fetch")), false);
});

test("relation groups are ordered and carry clickable identities", () => {
  const groups = relationGroups(
    context({
      ancestors: [
        { qualified_id: "web:WEB-001", title: "Epic", status: "To Do" },
        { qualified_id: "web:WEB-004", title: "Parent task", status: "Done" },
      ],
      children: [related("web:WEB-020", "Child task", "To Do")],
      dependencies: [related("web:WEB-005", "Dependency task", "Done")],
      dependents: [related("web:WEB-012", "Dependent task", "To Do")],
    }),
  );

  assert.deepEqual(
    groups.map((group) => group.heading),
    ["Parent chain", "Children", "Dependencies", "Dependents"],
  );
  assert.deepEqual(groups[0].chips, [
    { qualified_id: "web:WEB-001", title: "Epic", status: "To Do" },
    { qualified_id: "web:WEB-004", title: "Parent task", status: "Done" },
  ]);
  assert.deepEqual(groups[2].chips, [
    { qualified_id: "web:WEB-005", title: "Dependency task", status: "Done" },
  ]);
});

test("empty relations produce no group at all", () => {
  assert.deepEqual(relationGroups(context()), []);
});

test("the blocked-by chain lists every blocker naming another task", () => {
  const blocked = context({
    detail: {
      task: { qualified_id: "web:WEB-008", title: "Target task", status: "Blocked" },
      criteria: [],
    },
    readiness: {
      ready: false,
      blockers: [
        { kind: "task_status", task_id: "web:WEB-008", status: "Blocked" },
        { kind: "dependency", task_id: "web:WEB-005", status: "In Progress" },
        { kind: "ancestor_status", task_id: "web:WEB-001", status: "To Do" },
        { kind: "has_children" },
      ],
    },
  });

  assert.deepEqual(blockedByChain(blocked), [
    { qualified_id: "web:WEB-005", kind: "dependency", status: "In Progress" },
    { qualified_id: "web:WEB-001", kind: "ancestor_status", status: "To Do" },
  ]);
});

test("the blocked-by chain is empty unless the task itself is Blocked", () => {
  const ready = context({
    readiness: {
      ready: false,
      blockers: [{ kind: "dependency", task_id: "web:WEB-005", status: "In Progress" }],
    },
  });

  assert.deepEqual(blockedByChain(ready), []);
});

test("only block, defer, and obsolete take a reason", () => {
  assert.deepEqual(REASON_ACTIONS, ["block", "defer", "obsolete"]);
});
