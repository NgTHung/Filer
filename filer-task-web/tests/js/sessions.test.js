import assert from "node:assert/strict";
import test from "node:test";

import { sessionRequestFailed, sessionRequestSucceeded } from "../../static/js/lib/sessions.js";

test("a successful refresh clears an earlier session request error", () => {
  const rows = [{ id: 1 }];
  const failed = sessionRequestFailed(rows, new Error("offline"));
  const refreshed = sessionRequestSucceeded(rows);

  assert.equal(failed.error.message, "offline");
  assert.equal(refreshed.error, null);
});
