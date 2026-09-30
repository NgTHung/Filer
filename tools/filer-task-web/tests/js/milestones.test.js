import assert from "node:assert/strict";
import { test } from "node:test";

import { milestoneOptions, progressPercent, statusGroups } from "../../static/js/lib/milestones.js";

function aggregation(name) {
  return { milestone: { milestone: name }, tasks_by_status: {} };
}

test("the offered milestones are the aggregation's own names", () => {
  const aggregations = [aggregation("0.2.0"), aggregation("0.3.0")];

  assert.deepEqual(milestoneOptions(aggregations, ""), ["0.2.0", "0.3.0"]);
  assert.deepEqual(milestoneOptions([], ""), []);
  assert.deepEqual(milestoneOptions(undefined, ""), []);
});

test("a current milestone the aggregation no longer lists is still offered first", () => {
  assert.deepEqual(milestoneOptions([aggregation("0.3.0")], "0.1.0"), ["0.1.0", "0.3.0"]);
});

test("a current milestone the aggregation already lists is not duplicated", () => {
  assert.deepEqual(milestoneOptions([aggregation("0.3.0")], "0.3.0"), ["0.3.0"]);
});

test("progress is a whole percent of the aggregation's own counts", () => {
  assert.equal(progressPercent(1, 3), 33);
  assert.equal(progressPercent(2, 3), 67);
  assert.equal(progressPercent(3, 3), 100);
  assert.equal(progressPercent(0, 4), 0);
});

test("a milestone with no tasks is zero percent, not a division by zero", () => {
  assert.equal(progressPercent(0, 0), 0);
});

test("alphabetical map keys are regrouped into lifecycle order", () => {
  const groups = statusGroups({
    Blocked: [{ id: "A" }],
    Done: [{ id: "B" }],
    "In Progress": [{ id: "C" }],
    "To Do": [{ id: "D" }],
  });

  assert.deepEqual(
    groups.map(([status]) => status),
    ["To Do", "In Progress", "Blocked", "Done"],
  );
  assert.deepEqual(groups[0][1], [{ id: "D" }]);
});

test("statuses the aggregation omitted are not invented", () => {
  assert.deepEqual(
    statusGroups({ Done: [{ id: "B" }] }).map(([status]) => status),
    ["Done"],
  );
  assert.deepEqual(statusGroups({}), []);
  assert.deepEqual(statusGroups(undefined), []);
});

test("an empty group is dropped rather than rendered as an empty column", () => {
  assert.deepEqual(
    statusGroups({ "To Do": [], Done: [{ id: "B" }] }).map(([status]) => status),
    ["Done"],
  );
});

test("a status outside the lifecycle sorts after the known ones", () => {
  const groups = statusGroups({ Archived: [{ id: "A" }], "To Do": [{ id: "B" }], Zeta: [{ id: "C" }] });

  assert.deepEqual(
    groups.map(([status]) => status),
    ["To Do", "Archived", "Zeta"],
  );
});
