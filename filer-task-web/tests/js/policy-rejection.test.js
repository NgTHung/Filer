import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import { policyRejection, sectionRejection } from "../../static/js/lib/policyRejection.js";

function issue(code, task, extra = {}) {
  return {
    code,
    path: `/repo/.tasks/core/${task ?? "none"}.md`,
    message: `${code} on ${task ?? "the project"}`,
    context: task ? { task, ...extra } : { ...extra },
  };
}

test("two issues naming the same task collapse to one blocking id", () => {
  const rejected = new ApiError(422, {
    error: "task validation failed with 2 error(s)",
    code: "validation_failed",
    issues: [issue("tag_rejected", "core:CORE-001"), issue("tag_rejected", "core:CORE-001")],
  });

  assert.deepEqual(policyRejection(rejected).blockingTasks, ["core:CORE-001"]);
});

test("issues naming different tasks keep both in order", () => {
  const rejected = new ApiError(422, {
    error: "task validation failed with 2 error(s)",
    code: "validation_failed",
    issues: [issue("tag_rejected", "web:WEB-003"), issue("tag_rejected", "core:CORE-001")],
  });

  assert.deepEqual(policyRejection(rejected).blockingTasks, ["web:WEB-003", "core:CORE-001"]);
});

test("an issue naming no task still contributes its detail but no blocking id", () => {
  const broken = new ApiError(422, {
    error: "project filer failed validation",
    project: "filer",
    issues: [issue("config_unreadable", null)],
  });

  const rejection = policyRejection(broken);

  assert.deepEqual(rejection.blockingTasks, []);
  assert.equal(rejection.issues.length, 1);
  assert.equal(rejection.issues[0].code, "config_unreadable");
  assert.equal(rejection.issues[0].message, "config_unreadable on the project");
  assert.equal(rejection.issues[0].path, "/repo/.tasks/core/none.md");
});

test("a rejection carrying no issue list is a message on its own", () => {
  const duplicate = new ApiError(422, {
    error: 'domain "backend" is already configured',
    code: "config_duplicate",
  });

  assert.deepEqual(policyRejection(duplicate), {
    message: 'domain "backend" is already configured',
    blockingTasks: [],
    allowed: [],
    issues: [],
  });
});

test("a duplicate registration is a message on its own", () => {
  const duplicate = new ApiError(400, {
    error: 'project name "Filer" is registered more than once',
    code: "duplicate_project_name",
    project: "Filer",
  });

  assert.deepEqual(policyRejection(duplicate), {
    message: 'project name "Filer" is registered more than once',
    blockingTasks: [],
    allowed: [],
    issues: [],
  });
});

test("an allowed list is surfaced from the top level or the first issue that has one", () => {
  const topLevel = new ApiError(422, {
    error: "tag rejected is not in the catalog",
    code: "tag_rejected",
    context: { allowed: ["web", "tasks"] },
  });
  const perIssue = new ApiError(422, {
    error: "task validation failed with 1 error(s)",
    code: "validation_failed",
    issues: [issue("prefix_not_allowed", "core:CORE-001", { allowed: ["ALT"] })],
  });

  assert.deepEqual(policyRejection(topLevel).allowed, ["web", "tasks"]);
  assert.deepEqual(policyRejection(perIssue).allowed, ["ALT"]);
});

test("a rejection reaches only the section it was tagged with", () => {
  const rejection = { section: "tags", message: "no", blockingTasks: [], allowed: [], issues: [] };

  assert.equal(sectionRejection(rejection, "tags"), rejection);
  assert.equal(sectionRejection(rejection, "domains"), null);
  assert.equal(sectionRejection(rejection, "domain:web"), null);
  assert.equal(sectionRejection(null, "tags"), null);
});

test("a network failure is a message on its own", () => {
  assert.deepEqual(policyRejection(new TypeError("Failed to fetch")), {
    message: "Failed to fetch",
    blockingTasks: [],
    allowed: [],
    issues: [],
  });
});
