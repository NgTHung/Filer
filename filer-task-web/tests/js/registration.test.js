import assert from "node:assert/strict";
import { test } from "node:test";

import { projectRootPreview, registrationOutcome } from "../../static/js/lib/registration.js";

test("a path that resolves to a registered project switches to it", () => {
  assert.deepEqual(
    registrationOutcome({ code: "duplicate_project_name", project: "filer" }, false),
    { action: "switch", project: "filer" },
  );
});

test("creating where a project already exists falls back to opening it", () => {
  assert.deepEqual(registrationOutcome({ code: "project_already_exists" }, true), {
    action: "open",
  });
});

test("an existing project reported to a plain open is a refusal, not a retry", () => {
  assert.deepEqual(registrationOutcome({ code: "project_already_exists" }, false), {
    action: "reject",
  });
});

test("a duplicate without a name to switch to is a refusal", () => {
  assert.deepEqual(registrationOutcome({ code: "duplicate_project_name", project: null }, true), {
    action: "reject",
  });
});

test("the preview joins the location and the name into the root to be created", () => {
  assert.equal(projectRootPreview("D:/work", "new-thing"), "D:/work/new-thing");
  assert.equal(projectRootPreview("  D:/work  ", "  new-thing  "), "D:/work/new-thing");
});

test("the preview does not double a separator the location already ends with", () => {
  assert.equal(projectRootPreview("D:/work/", "new-thing"), "D:/work/new-thing");
  assert.equal(projectRootPreview("D:\\work\\", "new-thing"), "D:\\work/new-thing");
});

test("the preview marks a half-filled form rather than reading as a real path", () => {
  assert.equal(projectRootPreview("", ""), "…/…");
  assert.equal(projectRootPreview("D:/work", ""), "D:/work/…");
});

test("any other refusal is reported as-is", () => {
  assert.deepEqual(registrationOutcome({ code: "project_not_found" }, true), { action: "reject" });
  assert.deepEqual(registrationOutcome({ code: null }, false), { action: "reject" });
});
