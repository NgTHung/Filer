import assert from "node:assert/strict";
import { test } from "node:test";

import { activityLabel } from "../../static/js/lib/activity.js";

test("membership actions read as phrases and task actions keep their names", () => {
  assert.equal(activityLabel("member.grant"), "granted access");
  assert.equal(activityLabel("member.role"), "changed role");
  assert.equal(activityLabel("member.revoke"), "removed member");
  assert.equal(activityLabel("access.request"), "requested access");
  assert.equal(activityLabel("access.deny"), "denied access");
  assert.equal(activityLabel("task.done"), "task.done");
});
