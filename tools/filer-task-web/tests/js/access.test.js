import assert from "node:assert/strict";
import { test } from "node:test";

import { ACTIONS, accessBanner, can, isLastOwner } from "../../static/js/lib/access.js";

// One row per line of the spec's permissions table.
const TABLE = [
  [ACTIONS.REQUEST_ACCESS, { none: true, pending: false, editor: false, owner: false }],
  [ACTIONS.WRITE_TASKS, { none: false, pending: false, editor: true, owner: true }],
  [ACTIONS.MANAGE_MEMBERS, { none: false, pending: false, editor: false, owner: true }],
  [ACTIONS.EDIT_POLICY, { none: false, pending: false, editor: false, owner: true }],
];

test("project permissions follow the role, not the admin flag", () => {
  for (const [action, expected] of TABLE) {
    for (const isAdmin of [false, true]) {
      assert.equal(can(null, isAdmin, action), expected.none, `${action} for a non-member`);
      assert.equal(can("pending", isAdmin, action), expected.pending, `${action} while pending`);
      assert.equal(can("editor", isAdmin, action), expected.editor, `${action} for an editor`);
      assert.equal(can("owner", isAdmin, action), expected.owner, `${action} for an owner`);
    }
  }
});

test("only admins register projects, whatever their project role", () => {
  for (const role of [null, "pending", "editor", "owner"]) {
    assert.equal(can(role, true, ACTIONS.REGISTER_PROJECTS), true);
    assert.equal(can(role, false, ACTIONS.REGISTER_PROJECTS), false);
  }
});

test("the banner asks non-members to request access and shows a pending request", () => {
  assert.equal(accessBanner(null), "request");
  assert.equal(accessBanner(undefined), "request");
  assert.equal(accessBanner("pending"), "pending");
  assert.equal(accessBanner("editor"), null);
  assert.equal(accessBanner("owner"), null);
});

test("only the sole owner counts as the last owner", () => {
  const alone = [{ role: "owner" }, { role: "editor" }];
  const shared = [{ role: "owner" }, { role: "owner" }];
  assert.equal(isLastOwner(alone[0], alone), true);
  assert.equal(isLastOwner(alone[1], alone), false);
  assert.equal(isLastOwner(shared[0], shared), false);
});
