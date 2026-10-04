import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError, onAccessRejected, projectScoped } from "../../static/js/api/client.js";

function respondWith(status, body) {
  globalThis.fetch = async () => ({
    ok: status >= 200 && status < 300,
    status,
    text: async () => (body === null ? "" : JSON.stringify(body)),
  });
}

test("a missing-role refusal notifies the access handler and still rejects", async () => {
  const seen = [];
  onAccessRejected((error) => seen.push(error.code));
  respondWith(403, {
    error: "this change needs the editor role on the project",
    code: "project_role_required",
  });

  await assert.rejects(
    projectScoped("demo").transition("CORE-001", "start"),
    (error) => error instanceof ApiError && error.status === 403,
  );
  assert.deepEqual(seen, ["project_role_required"]);
});

test("other refusals leave the access handler alone", async () => {
  const seen = [];
  onAccessRejected((error) => seen.push(error.code));
  respondWith(409, { error: "a project must keep at least one owner", code: "last_owner" });

  await assert.rejects(projectScoped("demo").removeMember(1));
  assert.deepEqual(seen, []);
});
