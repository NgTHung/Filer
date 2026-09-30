import assert from "node:assert/strict";
import { test } from "node:test";

import { editDraft, editPatch, localizeRef } from "../../static/js/lib/taskEdit.js";

function detail(task, sections = []) {
  return {
    task: { domain: "web", qualified_id: `web:${task.id ?? "WEB-001"}`, ...task },
    sections,
    criteria_heading: "Acceptance Criteria",
    criteria: [],
  };
}

function fullDetail() {
  return detail(
    {
      id: "WEB-012",
      title: "Add task field editing",
      type: "Feature",
      status: "To Do",
      priority: "Medium",
      parent: "web:WEB-001",
      milestone: "0.3.0",
      depends_on: ["web:WEB-008", "core:CORE-014"],
      risk: "Low",
      impact: "Closes the edit gap.",
      tags: ["web", "tasks"],
    },
    [{ heading: "Summary", content: "The drawer renders fields read-only." }],
  );
}

test("a draft is filled from every field the detail carries", () => {
  assert.deepEqual(editDraft(fullDetail()), {
    title: "Add task field editing",
    summary: "The drawer renders fields read-only.",
    risk: "Low",
    impact: "Closes the edit gap.",
    tagsText: "web, tasks",
    milestone: "0.3.0",
    parent: "WEB-001",
    dependsText: "WEB-008, core:CORE-014",
  });
});

test("fields the server omitted become empty inputs rather than undefined", () => {
  const draft = editDraft(detail({ id: "WEB-004", title: "Bare task" }));

  assert.deepEqual(draft, {
    title: "Bare task",
    summary: "",
    risk: "",
    impact: "",
    tagsText: "",
    milestone: "",
    parent: "",
    dependsText: "",
  });
});

test("the summary comes from the Summary section and other sections are left alone", () => {
  const sections = [
    { heading: "Notes", content: "Not the summary." },
    { heading: "Summary", content: "The real summary." },
  ];

  assert.equal(editDraft(detail({ title: "T" }, sections)).summary, "The real summary.");
});

test("same-domain references lose their prefix and cross-domain ones keep it", () => {
  assert.equal(localizeRef("web:WEB-008", "web"), "WEB-008");
  assert.equal(localizeRef("core:CORE-014", "web"), "core:CORE-014");
  assert.equal(localizeRef("WEB-008", "web"), "WEB-008");
  assert.equal(localizeRef(null, "web"), "");
});

test("an untouched draft produces an empty patch", () => {
  const baseline = editDraft(fullDetail());

  assert.deepEqual(editPatch({ ...baseline }, baseline), {});
});

test("only the fields the user changed reach the patch", () => {
  const baseline = editDraft(fullDetail());

  assert.deepEqual(editPatch({ ...baseline, impact: "A new impact." }, baseline), {
    impact: "A new impact.",
  });
});

test("emptying a nullable field clears it with an explicit null", () => {
  const baseline = editDraft(fullDetail());
  const cleared = { ...baseline, risk: "", impact: "", milestone: "", parent: "" };

  assert.deepEqual(editPatch(cleared, baseline), {
    risk: null,
    impact: null,
    milestone: null,
    parent: null,
  });
});

test("emptying a list field clears it with an empty array, never null", () => {
  const baseline = editDraft(fullDetail());
  const cleared = { ...baseline, tagsText: "", dependsText: "  ,  " };

  assert.deepEqual(editPatch(cleared, baseline), { tags: [], depends_on: [] });
});

test("list fields are re-parsed and trimmed, and reordering counts as a change", () => {
  const baseline = editDraft(fullDetail());

  assert.deepEqual(editPatch({ ...baseline, tagsText: " tasks , web " }, baseline), {
    tags: ["tasks", "web"],
  });
  assert.deepEqual(editPatch({ ...baseline, tagsText: "web,  tasks" }, baseline), {});
});

test("surrounding whitespace alone is not a change", () => {
  const baseline = editDraft(fullDetail());

  assert.deepEqual(editPatch({ ...baseline, title: "  Add task field editing  " }, baseline), {});
});

test("a rewritten title and summary are sent trimmed", () => {
  const baseline = editDraft(fullDetail());
  const edited = { ...baseline, title: "  Renamed  ", summary: "  Rewritten.  " };

  assert.deepEqual(editPatch(edited, baseline), { title: "Renamed", summary: "Rewritten." });
});

test("a dependency added by its bare id is sent as typed", () => {
  const baseline = editDraft(fullDetail());
  const edited = { ...baseline, dependsText: "WEB-008, core:CORE-014, WEB-010" };

  assert.deepEqual(editPatch(edited, baseline), {
    depends_on: ["WEB-008", "core:CORE-014", "WEB-010"],
  });
});
