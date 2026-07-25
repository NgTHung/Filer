# WEB-007: Milestones Screen and New-task Form Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the two screens the v1 UI never had: a Milestones progress view over `GET /api/projects/{project}/milestones`, and a policy-driven New-task form over `POST /api/projects/{project}/tasks` that surfaces rejections inline and opens the created task in a detail drawer.

**Architecture:** Backend is complete and unchanged. All three endpoints (`/milestones`, `/policy`, `POST /tasks`) already exist, are tested, and already carry the `field` mapping the form needs (`filer-task-web/src/error.rs:259-264`). This is a pure frontend change to the vendored Preact + htm app under `filer-task-web/static/`. Every decision that can be a pure function (slugging, next-number derivation, path preview, policy option reading, milestone status ordering, error-to-field mapping) moves into `static/js/lib/` modules covered by `node --test` through `tests/frontend_js_test.rs`. The screens themselves stay thin renderers.

**Tech Stack:** Preact 10 + htm, vendored under `static/vendor/`, no build step, ES modules loaded by `static/index.html`. Tests: Node's built-in test runner (`node:test` + `node:assert/strict`) over `tests/js/*.test.js`, driven from `cargo test -p filer-task-web`.

## Global Constraints

- No new dependencies. No build step. No npm packages. The vendored `preact-htm.js` shim is the only import surface for components.
- Every new pure module lives under `static/js/lib/` and gets a `tests/js/<name>.test.js` sibling. Screens and components under `static/js/screens/` and `static/js/components/` are not unit tested (there is no DOM harness in this repo); they must contain no logic that could live in a lib module.
- Follow the existing style exactly: `.js` extension on every relative import, `export function`, no default exports, no semicolonless lines, 2-space indent, double quotes, trailing commas in multiline literals.
- Comment rules from `AGENTS.md`: explain WHY, not WHAT. No section dividers. No placeholder comments. Module-level comments in `static/js/lib/` are plain `//` block headers (see `static/js/lib/filters.js:1-4` and `static/js/lib/sorting.js:1-5` for the house style).
- Duplicated logic is a code smell: Task 1 exists specifically to stop the tag-policy read being written a second time.
- Scope boundary agreed with the user: WEB-007 ships a **minimal** `TaskDrawer` that renders a `ShowView` it is handed. No fetch-by-id, no lifecycle actions, no criteria toggles, no relationship chips. WEB-008 extends this same component.
- Do not touch any Rust source.
- `filer-task-web/static/app.js` is the dead v1 bundle, superseded by `static/js/app.js`. Leave it alone; it is not this task's to delete.
- `static/style.css` is append-only here: add new rules at the end, never edit an existing one.
- The stylesheet's palette is CSS custom properties declared in `:root` at `style.css:1-11` — `--bg`, `--panel`, `--border`, `--text`, `--muted`, `--accent`, `--danger-bg`, `--danger-border`, `--danger-text`. Every new rule uses `var(--name)` and `px` units. No raw hex, no `rem`.

## Reference: response shapes this plan consumes

`GET /api/projects/{project}/policy` (`ProjectPolicyResponse`, `src/dto.rs:238-376`):

```json
{
  "domains": { "web": { "prefixes": ["WEB"] }, "core": { "prefixes": ["UTILS", "CORE"] } },
  "task_types": { "Feature": { "criteria": "acceptance" }, "Milestone": { "criteria": "exit", "role": "milestone" } },
  "tags": { "policy": "strict", "allowed": ["web", "tasks"] }
}
```

Under an open tag policy `tags` is `{ "policy": "open" }` with no `allowed` key.

`GET /api/projects/{project}/milestones` (`Vec<MilestoneAggregation>`, `filer-task/src/milestone.rs:28-36`):

```json
[
  {
    "milestone": { "path": "...", "domain": "milestones", "qualified_id": "milestones:MILESTONE-001",
                   "id": "MILESTONE-001", "title": "...", "status": "To Do", "priority": "High",
                   "type": "Milestone", "milestone": "0.3.0" },
    "criteria_heading": "Exit Criteria",
    "criteria": [{ "checked": false, "text": "Ships" }],
    "done": 1,
    "total": 3,
    "tasks_by_status": { "Blocked": [ /* TaskView */ ], "Done": [], "In Progress": [] }
  }
]
```

`tasks_by_status` is a Rust `BTreeMap`, so its keys arrive in **alphabetical** order (`Blocked`, `Deferred`, `Done`, `In Progress`, `Obsolete`, `To Do`) and empty statuses are absent entirely. Task 3 exists to restore lifecycle order.

`GET /api/projects/{project}/tasks` (`Vec<Task>`, `filer-task/src/model.rs:89-111`) — flattened metadata, so each row has `qualified_id`, `path`, `domain`, `id`, `title`, `status`, `priority`, `type`, and optionally `milestone`, `last_updated`, `tags`.

`POST /api/projects/{project}/tasks` returns `ShowView` (`filer-task/src/agent_context.rs:86-98`):

```json
{
  "schema_version": 2,
  "warnings": [],
  "detail": {
    "task": { "path": "...", "domain": "web", "qualified_id": "web:WEB-030", "id": "WEB-030", "title": "...", "status": "To Do", "priority": "Medium", "type": "Feature" },
    "sections": [{ "heading": "Summary", "content": "..." }],
    "criteria_heading": "Acceptance Criteria",
    "criteria": [{ "checked": false, "text": "...", "content_hash": "..." }]
  }
}
```

On rejection the body is `ErrorBody` (`src/error.rs:56-69`) with `field` already set: `id_exists` → `"number"`, `prefix_not_allowed` → `"prefix"`, `tag_rejected` → `"tags"`. `ApiError` in `static/js/api/client.js:3-13` already lifts `code`, `field`, and `context`. The client needs no new parsing, only routing.

## File Structure

**Create:**

| File | Responsibility |
|---|---|
| `filer-task-web/static/js/lib/policy.js` | Read option lists out of the policy response. Shared by FilterMenu and NewTask. |
| `filer-task-web/static/js/lib/newTask.js` | Derive the next number, the slug, the qualified id, the file path, and the field an error belongs to. |
| `filer-task-web/static/js/lib/milestones.js` | Progress percent and lifecycle-ordered status groups. |
| `filer-task-web/static/js/screens/Milestones.js` | Milestone card list. |
| `filer-task-web/static/js/screens/NewTask.js` | Creation form. |
| `filer-task-web/static/js/components/TaskDrawer.js` | Read-only right-side drawer over a `ShowView`. WEB-008 extends this. |
| `filer-task-web/tests/js/policy.test.js` | Tests for `lib/policy.js`. |
| `filer-task-web/tests/js/new-task.test.js` | Tests for `lib/newTask.js`. |
| `filer-task-web/tests/js/milestones.test.js` | Tests for `lib/milestones.js`. |

**Modify:**

| File | Change |
|---|---|
| `filer-task-web/static/js/components/FilterMenu.js:54-55` | Replace the inline tag-policy read with `tagCatalog` from `lib/policy.js`. |
| `filer-task-web/static/js/app.js:12-24, 26-72` | Route the `milestones` and `new-task` screens; hold drawer state. |
| `filer-task-web/static/style.css` | Append milestone card, form, and drawer rules. |
| `.tasks/web/WEB-007-build-the-v2-milestones-screen-and-new-task-form.md` | Check criteria at the end. |

Nothing under `filer-task-web/src/` changes.

## Testing reality

The repo has no DOM test harness, and adding jsdom would violate the no-new-dependencies constraint. So:

- Tasks 1-3 are strict TDD: failing test first, then the module.
- Tasks 4-6 render already-tested logic and are verified by the manual browser pass in Task 7, which walks each acceptance criterion against a real server.

Do not claim an acceptance criterion is met before Task 7 confirms it in a browser.

---

### Task 0: Start the task

**Files:** none (tracker state only)

- [ ] **Step 1: Confirm the task is still ready**

Run: `cargo run -q -p filer-task -- context web:WEB-007 --format json`
Expected: `readiness.ready` is `true` with an empty `blockers` array. If it is not, stop and report the blocker rather than starting.

- [ ] **Step 2: Start it**

Run: `cargo run -q -p filer-task -- start web:WEB-007`
Expected: status becomes `In Progress`. If it is already `In Progress`, skip this step; do not issue a second lifecycle mutation.

- [ ] **Step 3: Validate and commit the state change**

```bash
cargo run -q -p filer-task -- validate
rtk git add .tasks/web/WEB-007-build-the-v2-milestones-screen-and-new-task-form.md
rtk git commit -m "chore(tasks): start WEB-007"
```

---

### Task 1: Policy option readers

**Files:**
- Create: `filer-task-web/static/js/lib/policy.js`
- Create: `filer-task-web/tests/js/policy.test.js`
- Modify: `filer-task-web/static/js/components/FilterMenu.js:54-55`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `domainNames(policy) -> string[]` — sorted domain names, `[]` when policy is null/absent.
  - `prefixesFor(policy, domain) -> string[]` — prefixes for one domain, `[]` when unknown.
  - `taskTypeNames(policy) -> string[]` — sorted task type names, `[]` when absent.
  - `tagCatalog(policy) -> string[] | null` — the allowed list under a strict policy, `null` under an open policy or no policy.

- [ ] **Step 1: Write the failing tests**

Create `filer-task-web/tests/js/policy.test.js`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { domainNames, prefixesFor, tagCatalog, taskTypeNames } from "../../static/js/lib/policy.js";

const POLICY = {
  domains: { web: { prefixes: ["WEB"] }, core: { prefixes: ["UTILS", "CORE"] } },
  task_types: { Milestone: { criteria: "exit", role: "milestone" }, Feature: { criteria: "acceptance" } },
  tags: { policy: "strict", allowed: ["web", "tasks"] },
};

test("domains and task types are listed in a stable order", () => {
  assert.deepEqual(domainNames(POLICY), ["core", "web"]);
  assert.deepEqual(taskTypeNames(POLICY), ["Feature", "Milestone"]);
});

test("prefixes are scoped to one domain", () => {
  assert.deepEqual(prefixesFor(POLICY, "core"), ["UTILS", "CORE"]);
  assert.deepEqual(prefixesFor(POLICY, "web"), ["WEB"]);
});

test("an unknown or unset domain has no prefixes", () => {
  assert.deepEqual(prefixesFor(POLICY, "missing"), []);
  assert.deepEqual(prefixesFor(POLICY, ""), []);
});

test("a strict policy exposes its catalog and an open policy does not", () => {
  assert.deepEqual(tagCatalog(POLICY), ["web", "tasks"]);
  assert.equal(tagCatalog({ tags: { policy: "open" } }), null);
});

test("a strict policy with no allowed list is an empty catalog, not an open one", () => {
  assert.deepEqual(tagCatalog({ tags: { policy: "strict" } }), []);
});

test("a policy that has not loaded yet yields empty options", () => {
  for (const policy of [null, undefined, {}]) {
    assert.deepEqual(domainNames(policy), []);
    assert.deepEqual(taskTypeNames(policy), []);
    assert.deepEqual(prefixesFor(policy, "web"), []);
    assert.equal(tagCatalog(policy), null);
  }
});
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `node --test tests/js/policy.test.js` from `filer-task-web/`
Expected: FAIL with `Cannot find module ... static/js/lib/policy.js`

- [ ] **Step 3: Write the module**

Create `filer-task-web/static/js/lib/policy.js`:

```js
// Option lists for every policy-driven control. The policy response nests
// prefixes under their domain and only carries an allowed tag list under a
// strict policy, so each reader also answers for a policy that has not loaded.

export function domainNames(policy) {
  return Object.keys(policy?.domains ?? {}).sort();
}

export function taskTypeNames(policy) {
  return Object.keys(policy?.task_types ?? {}).sort();
}

export function prefixesFor(policy, domain) {
  return policy?.domains?.[domain]?.prefixes ?? [];
}

// Null distinguishes an open policy, where any tag is legal, from a strict one
// with an empty catalog, where none is.
export function tagCatalog(policy) {
  if (policy?.tags?.policy !== "strict") {
    return null;
  }
  return policy.tags.allowed ?? [];
}
```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `node --test tests/js/policy.test.js`
Expected: PASS, 6 tests

- [ ] **Step 5: Route FilterMenu through the shared reader**

In `filer-task-web/static/js/components/FilterMenu.js`, add to the imports at line 2:

```js
import { tagCatalog } from "../lib/policy.js";
```

Replace lines 54-55:

```js
  const strictTags = policy && policy.tags && policy.tags.policy === "strict";
  const tagOptions = strictTags ? policy.tags.allowed ?? [] : null;
```

with:

```js
  const tagOptions = tagCatalog(policy);
```

Leave the rest of the component alone: line 154 already branches on `tagOptions` being truthy, and `[]` is truthy, so a strict-but-empty catalog still renders the select rather than a free-text input. That is the correct behavior and it is now the tested one.

- [ ] **Step 6: Run the whole frontend suite**

Run: `rtk cargo test -p filer-task-web --test frontend_js_test`
Expected: PASS. If it prints `skipping frontend module tests: node not found on PATH`, Node is missing; install it before continuing, because every later task depends on this harness.

- [ ] **Step 7: Commit**

```bash
rtk git add filer-task-web/static/js/lib/policy.js filer-task-web/tests/js/policy.test.js filer-task-web/static/js/components/FilterMenu.js
rtk git commit -m "feat(task-web): share policy option readers across the filter menu"
```

---

### Task 2: New-task id derivation and error routing

**Files:**
- Create: `filer-task-web/static/js/lib/newTask.js`
- Create: `filer-task-web/tests/js/new-task.test.js`

**Interfaces:**
- Consumes: `ApiError` from `static/js/api/client.js`.
- Produces:
  - `slugify(title) -> string` — mirrors `filer-task/src/lifecycle.rs:536-546`.
  - `nextNumber(tasks, domain, prefix) -> string` — one past the highest existing number, zero-padded to the width of that highest id, `"001"` when none exist.
  - `preview(domain, prefix, number, title) -> { qualifiedId: string, path: string }` — mirrors `new_task_path` at `filer-task/src/lifecycle.rs:368-371`.
  - `fieldError(error) -> { field: string | null, message: string, allowed: string[] }` — `field` is null when the failure is not attributable to one input.

The path preview must match the server byte for byte, otherwise it lies to the user. `new_task_path` builds `<root>/.tasks/<domain>/<prefix>-<number>-<slug(title)>.md`; the preview omits the root because the client does not know it.

- [ ] **Step 1: Write the failing tests**

Create `filer-task-web/tests/js/new-task.test.js`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError } from "../../static/js/api/client.js";
import { fieldError, nextNumber, preview, slugify } from "../../static/js/lib/newTask.js";

function task(domain, id) {
  return { domain, id, qualified_id: `${domain}:${id}`, title: id, status: "To Do" };
}

test("the slug lowercases alphanumerics and collapses everything else to one dash", () => {
  assert.equal(slugify("Build the Milestones screen"), "build-the-milestones-screen");
  assert.equal(slugify("Serve ready-work  &  milestone endpoints"), "serve-ready-work-milestone-endpoints");
  assert.equal(slugify("Filer 2.0: what's next?"), "filer-2-0-what-s-next");
});

test("leading and trailing separators are trimmed off the slug", () => {
  assert.equal(slugify("  --Trailing punctuation!!  "), "trailing-punctuation");
  assert.equal(slugify("!!!"), "");
});

test("non-ascii characters become separators, matching the server", () => {
  assert.equal(slugify("Café résumé"), "caf-r-sum");
});

test("the next number is one past the highest id sharing the domain and prefix", () => {
  const tasks = [task("web", "WEB-001"), task("web", "WEB-021"), task("web", "WEB-007")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "022");
});

test("other domains and other prefixes do not raise the number", () => {
  const tasks = [task("core", "WEB-900"), task("web", "UTILS-500"), task("web", "WEB-003")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "004");
});

test("an unused domain and prefix pair starts at 001", () => {
  assert.equal(nextNumber([task("web", "WEB-004")], "core", "UTILS"), "001");
  assert.equal(nextNumber([], "web", "WEB"), "001");
});

test("padding follows the width of the highest existing id", () => {
  assert.equal(nextNumber([task("web", "WEB-0007")], "web", "WEB"), "0008");
  assert.equal(nextNumber([task("web", "WEB-9")], "web", "WEB"), "10");
});

test("ids whose suffix is not a plain number are ignored", () => {
  const tasks = [task("web", "WEB-A12"), task("web", "WEB-1-2"), task("web", "WEB-002")];

  assert.equal(nextNumber(tasks, "web", "WEB"), "003");
});

test("the preview shows the qualified id and the path the server will write", () => {
  assert.deepEqual(preview("web", "WEB", "030", "Build the Milestones screen"), {
    qualifiedId: "web:WEB-030",
    path: ".tasks/web/WEB-030-build-the-milestones-screen.md",
  });
});

test("a title with no slug-able characters still previews a valid path", () => {
  assert.deepEqual(preview("web", "WEB", "030", ""), {
    qualifiedId: "web:WEB-030",
    path: ".tasks/web/WEB-030-.md",
  });
});

test("an incomplete form previews nothing rather than a wrong path", () => {
  assert.equal(preview("", "WEB", "030", "Title"), null);
  assert.equal(preview("web", "", "030", "Title"), null);
  assert.equal(preview("web", "WEB", "", "Title"), null);
});

test("a rejection is routed to the field the server named", () => {
  const duplicate = new ApiError(422, {
    error: "task core:CORE-007 already exists",
    code: "id_exists",
    field: "number",
    context: { task: "core:CORE-007" },
  });

  assert.deepEqual(fieldError(duplicate), {
    field: "number",
    message: "task core:CORE-007 already exists",
    allowed: [],
  });
});

test("a rejected tag carries the catalog so the form can offer it", () => {
  const rejected = new ApiError(422, {
    error: "tag rejected is not in the catalog",
    code: "tag_rejected",
    field: "tags",
    context: { rejected_value: "rejected", allowed: ["web", "tasks"] },
  });

  assert.deepEqual(fieldError(rejected), {
    field: "tags",
    message: "tag rejected is not in the catalog",
    allowed: ["web", "tasks"],
  });
});

test("an error naming no field falls back to a form-level message", () => {
  const broken = new ApiError(422, { error: "project filer failed validation", project: "filer" });
  const offline = new TypeError("Failed to fetch");

  assert.deepEqual(fieldError(broken), {
    field: null,
    message: "project filer failed validation",
    allowed: [],
  });
  assert.deepEqual(fieldError(offline), { field: null, message: "Failed to fetch", allowed: [] });
});
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `node --test tests/js/new-task.test.js` from `filer-task-web/`
Expected: FAIL with `Cannot find module ... static/js/lib/newTask.js`

- [ ] **Step 3: Write the module**

Create `filer-task-web/static/js/lib/newTask.js`:

```js
// Everything the creation form derives before it posts. The slug and the path
// mirror new_task_path and slug in filer-task/src/lifecycle.rs, so the preview
// names the file the server will actually write; changing either there without
// changing this makes the preview lie.

import { ApiError } from "../api/client.js";

const ALPHANUMERIC = /^[0-9A-Za-z]$/;
const DIGITS = /^[0-9]+$/;

export function slugify(title) {
  let slug = "";
  for (const character of title) {
    if (ALPHANUMERIC.test(character)) {
      slug += character.toLowerCase();
    } else if (!slug.endsWith("-")) {
      slug += "-";
    }
  }
  return slug.replace(/^-+/, "").replace(/-+$/, "");
}

// Padding copies the widest existing id rather than a fixed width, so a project
// numbering past 999 keeps its own convention instead of being reset to three.
export function nextNumber(tasks, domain, prefix) {
  let highest = 0;
  let width = 3;
  for (const task of tasks ?? []) {
    if (task.domain !== domain || !task.id.startsWith(`${prefix}-`)) {
      continue;
    }
    const suffix = task.id.slice(prefix.length + 1);
    if (!DIGITS.test(suffix)) {
      continue;
    }
    const value = Number(suffix);
    if (value >= highest) {
      highest = value;
      width = suffix.length;
    }
  }
  return String(highest + 1).padStart(width, "0");
}

export function preview(domain, prefix, number, title) {
  if (!domain || !prefix || !number) {
    return null;
  }
  const id = `${prefix}-${number}`;
  return {
    qualifiedId: `${domain}:${id}`,
    path: `.tasks/${domain}/${id}-${slugify(title ?? "")}.md`,
  };
}

// The server already names the offending input on the error body, so the form
// only routes it; anything unattributed belongs above the form, not beside an
// arbitrary field.
export function fieldError(error) {
  if (!(error instanceof ApiError)) {
    return { field: null, message: error.message, allowed: [] };
  }
  const allowed = error.context && error.context.allowed;
  return {
    field: error.field ?? null,
    message: error.message,
    allowed: Array.isArray(allowed) ? allowed : [],
  };
}
```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `node --test tests/js/new-task.test.js`
Expected: PASS, 13 tests

- [ ] **Step 5: Commit**

```bash
rtk git add filer-task-web/static/js/lib/newTask.js filer-task-web/tests/js/new-task.test.js
rtk git commit -m "feat(task-web): derive new-task ids, paths, and error fields"
```

---

### Task 3: Milestone progress and status ordering

**Files:**
- Create: `filer-task-web/static/js/lib/milestones.js`
- Create: `filer-task-web/tests/js/milestones.test.js`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `progressPercent(done, total) -> number` — 0-100, integer, 0 when total is 0.
  - `statusGroups(tasksByStatus) -> Array<[string, object[]]>` — non-empty groups in lifecycle order, unknown statuses appended alphabetically.

`STATUS_ORDER` matches the lifecycle order the `filer-task` CLI uses, and the same list already appears as `STATUS_OPTIONS` in `FilterMenu.js:4`. Keep them consistent; do not import one from the other, because the filter list is a query vocabulary and this one is a display order, and they are free to diverge.

- [ ] **Step 1: Write the failing tests**

Create `filer-task-web/tests/js/milestones.test.js`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { progressPercent, statusGroups } from "../../static/js/lib/milestones.js";

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
```

- [ ] **Step 2: Run the tests and verify they fail**

Run: `node --test tests/js/milestones.test.js` from `filer-task-web/`
Expected: FAIL with `Cannot find module ... static/js/lib/milestones.js`

- [ ] **Step 3: Write the module**

Create `filer-task-web/static/js/lib/milestones.js`:

```js
// Presentation shape for one milestone aggregation. tasks_by_status arrives as
// a Rust BTreeMap, so its keys are alphabetical and empty statuses are absent;
// both need fixing before a reader can scan a milestone top to bottom.

const STATUS_ORDER = ["To Do", "In Progress", "Blocked", "Done", "Deferred", "Obsolete"];

export function progressPercent(done, total) {
  if (!total) {
    return 0;
  }
  return Math.round((done / total) * 100);
}

export function statusGroups(tasksByStatus) {
  const entries = Object.entries(tasksByStatus ?? {}).filter(([, tasks]) => tasks.length > 0);
  return entries.sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b));
}

// An unconfigured status still renders, after the lifecycle it does not belong to.
function rank(status) {
  const index = STATUS_ORDER.indexOf(status);
  return index === -1 ? STATUS_ORDER.length : index;
}
```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `node --test tests/js/milestones.test.js`
Expected: PASS, 6 tests

- [ ] **Step 5: Run the whole frontend suite**

Run: `rtk cargo test -p filer-task-web --test frontend_js_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
rtk git add filer-task-web/static/js/lib/milestones.js filer-task-web/tests/js/milestones.test.js
rtk git commit -m "feat(task-web): order milestone status groups by lifecycle"
```

---

### Task 4: Milestones screen

Delivers acceptance criterion 1.

**Files:**
- Create: `filer-task-web/static/js/screens/Milestones.js`
- Modify: `filer-task-web/static/js/app.js:12-24`
- Modify: `filer-task-web/static/style.css` (append)

**Interfaces:**
- Consumes: `progressPercent`, `statusGroups` from `../lib/milestones.js`; `projectScoped` from `../api/client.js`; `Header` from `../components/Header.js`.
- Produces: `MilestonesScreen({ projectName })`.

Model it on `screens/Ready.js:17-113`: `useMemo` for the api, a `load` function that sets rows or the error, a `useEffect` keyed on `projectName`, `<${Header} title=... onRefresh=${load} />`, and an `.empty-state` paragraph when there is nothing.

- [ ] **Step 1: Write the screen**

Create `filer-task-web/static/js/screens/Milestones.js`:

```js
import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";
import { progressPercent, statusGroups } from "../lib/milestones.js";

export function MilestonesScreen({ projectName }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [rows, setRows] = useState([]);
  const [error, setError] = useState(null);

  async function load() {
    try {
      setRows(await api.getMilestones());
      setError(null);
    } catch (err) {
      setError(err);
    }
  }

  useEffect(() => {
    load();
  }, [projectName]);

  return html`
    <section class="screen">
      <${Header} title="Milestones" onRefresh=${load} />
      ${error ? html`<p class="screen-error">Could not load milestones: ${error.message}</p>` : null}
      ${rows.length === 0 && !error
        ? html`<p class="empty-state">No milestone-role tasks are declared in this project.</p>`
        : null}
      ${rows.map((row) => html`<${MilestoneCard} key=${row.milestone.qualified_id} aggregation=${row} />`)}
    </section>
  `;
}

function MilestoneCard({ aggregation }) {
  const { milestone, criteria, criteria_heading, done, total } = aggregation;
  const percent = progressPercent(done, total);

  return html`
    <article class="milestone-card">
      <header class="milestone-card-header">
        <h3>${milestone.milestone ?? milestone.id}</h3>
        <span class="milestone-title">${milestone.title}</span>
        <span class="milestone-status">${milestone.status}</span>
      </header>
      <div class="milestone-progress" role="progressbar" aria-valuenow=${percent} aria-valuemin="0" aria-valuemax="100">
        <div class="milestone-progress-fill" style=${`width: ${percent}%`}></div>
      </div>
      <p class="milestone-progress-label">${done} of ${total} done (${percent}%)</p>
      <h4>${criteria_heading}</h4>
      ${criteria.length === 0
        ? html`<p class="milestone-empty">No ${criteria_heading.toLowerCase()} listed.</p>`
        : html`
            <ul class="milestone-criteria">
              ${criteria.map(
                (item, index) => html`
                  <li key=${index} class=${item.checked ? "criterion-checked" : ""}>
                    <span class="criterion-marker">${item.checked ? "✓" : "○"}</span>
                    ${item.text}
                  </li>
                `,
              )}
            </ul>
          `}
      ${statusGroups(aggregation.tasks_by_status).map(
        ([status, tasks]) => html`
          <div key=${status} class="milestone-group">
            <h4>${status} <span class="milestone-group-count">${tasks.length}</span></h4>
            <ul class="milestone-task-list">
              ${tasks.map(
                (task) => html`
                  <li key=${task.qualified_id}>
                    <span class="milestone-task-id">${task.qualified_id}</span>
                    <span class="milestone-task-title">${task.title}</span>
                  </li>
                `,
              )}
            </ul>
          </div>
        `,
      )}
    </article>
  `;
}
```

- [ ] **Step 2: Route the screen**

In `filer-task-web/static/js/app.js`, add to the imports after line 6:

```js
import { MilestonesScreen } from "./screens/Milestones.js";
```

Replace lines 19-23:

```js
  if (screen === "tasks") {
    return html`<${TasksScreen} projectName=${project.name} />`;
  }
  // Milestones and New task ship in WEB-007.
  return html`<p class="empty-state">This screen is not built yet.</p>`;
```

with:

```js
  if (screen === "tasks") {
    return html`<${TasksScreen} projectName=${project.name} />`;
  }
  if (screen === "milestones") {
    return html`<${MilestonesScreen} projectName=${project.name} />`;
  }
  // New task ships later in WEB-007.
  return html`<p class="empty-state">This screen is not built yet.</p>`;
```

- [ ] **Step 3: Append the styles**

Append to `filer-task-web/static/style.css`:

```css
.milestone-card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 16px;
  margin-bottom: 16px;
}

.milestone-card-header {
  display: flex;
  align-items: baseline;
  gap: 12px;
  flex-wrap: wrap;
}

.milestone-card-header h3 {
  margin: 0;
  font-size: 16px;
}

.milestone-title {
  color: var(--muted);
}

.milestone-status {
  margin-left: auto;
  font-size: 12px;
  color: var(--muted);
}

.milestone-progress {
  height: 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg);
  overflow: hidden;
  margin: 12px 0 6px;
}

.milestone-progress-fill {
  height: 100%;
  background: var(--accent);
}

.milestone-progress-label {
  margin: 0 0 12px;
  font-size: 12px;
  color: var(--muted);
}

.milestone-card h4 {
  margin: 12px 0 6px;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted);
}

.milestone-criteria,
.milestone-task-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.milestone-criteria li,
.milestone-task-list li {
  display: flex;
  gap: 8px;
  padding: 2px 0;
}

.criterion-checked {
  color: var(--muted);
}

.criterion-marker {
  width: 14px;
}

.milestone-group-count {
  color: var(--muted);
}

.milestone-task-id {
  min-width: 180px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
}

.milestone-empty {
  margin: 0;
  color: var(--muted);
}
```

- [ ] **Step 4: See it render**

Run: `rtk cargo run -p filer-task-web -- .` and open `http://127.0.0.1:3000` (the bound address is printed on startup; use whatever it prints).
Click **Milestones**. Expected: one card per milestone-role task in `.tasks/milestones/`, each showing a filled progress bar, its criteria list, and its tasks grouped `To Do`, `In Progress`, `Blocked`, `Done`. Stop the server with Ctrl-C.

- [ ] **Step 5: Commit**

```bash
rtk git add filer-task-web/static/js/screens/Milestones.js filer-task-web/static/js/app.js filer-task-web/static/style.css
rtk git commit -m "feat(task-web): add the milestones screen"
```

---

### Task 5: Read-only task drawer

Delivers the drawer half of acceptance criterion 5. WEB-008 extends this component rather than replacing it.

**Files:**
- Create: `filer-task-web/static/js/components/TaskDrawer.js`
- Modify: `filer-task-web/static/style.css` (append)

**Interfaces:**
- Consumes: a `ShowView` object.
- Produces: `TaskDrawer({ view, onClose })` — renders `view.detail`. Renders nothing when `view` is null, so a caller can pass its state straight through.

Deliberately out of scope, per the scope boundary in Global Constraints: fetching by id, the five lifecycle actions, criteria toggles with `If-Match`, relationship chips, and the blocked-by chain. Those are WEB-008 acceptance criteria.

- [ ] **Step 1: Write the component**

Create `filer-task-web/static/js/components/TaskDrawer.js`:

```js
import { html } from "../../vendor/preact-htm.js";

// Read-only view over a ShowView the caller already has. Lifecycle actions and
// criteria toggles land in WEB-008 on this same component, so it takes the view
// as a prop instead of fetching, and every caller keeps owning its own refresh.
export function TaskDrawer({ view, onClose }) {
  if (!view) {
    return null;
  }
  const { task, sections, criteria, criteria_heading } = view.detail;

  return html`
    <div class="drawer-overlay" onClick=${onClose}>
      <aside class="drawer" onClick=${(event) => event.stopPropagation()}>
        <header class="drawer-header">
          <span class="drawer-id">${task.qualified_id}</span>
          <span class="drawer-badge">${task.status}</span>
          <span class="drawer-badge">${task.priority}</span>
          <button class="drawer-close" onClick=${onClose} title="Close">×</button>
        </header>
        <h3 class="drawer-title">${task.title}</h3>
        <dl class="drawer-meta">
          <dt>Type</dt>
          <dd>${task.type}</dd>
          <dt>Domain</dt>
          <dd>${task.domain}</dd>
          ${task.milestone ? html`<dt>Milestone</dt><dd>${task.milestone}</dd>` : null}
          ${(task.tags ?? []).length > 0 ? html`<dt>Tags</dt><dd>${task.tags.join(", ")}</dd>` : null}
          <dt>Path</dt>
          <dd class="drawer-path">${task.path}</dd>
        </dl>
        ${sections.map(
          (section) => html`
            <section key=${section.heading} class="drawer-section">
              <h4>${section.heading}</h4>
              <p>${section.content}</p>
            </section>
          `,
        )}
        <section class="drawer-section">
          <h4>${criteria_heading}</h4>
          ${criteria.length === 0
            ? html`<p class="milestone-empty">None listed.</p>`
            : html`
                <ul class="milestone-criteria">
                  ${criteria.map(
                    (item) => html`
                      <li key=${item.content_hash} class=${item.checked ? "criterion-checked" : ""}>
                        <span class="criterion-marker">${item.checked ? "✓" : "○"}</span>
                        ${item.text}
                      </li>
                    `,
                  )}
                </ul>
              `}
        </section>
        ${view.warnings.length > 0
          ? html`
              <section class="drawer-section">
                <h4>Warnings</h4>
                <ul class="issue-list">
                  ${view.warnings.map((warning, index) => html`<li key=${index}>${warning.message}</li>`)}
                </ul>
              </section>
            `
          : null}
      </aside>
    </div>
  `;
}
```

If `ValidationWarning` turns out not to expose `message`, check its definition in `filer-task/src/validate.rs` and render the field it does expose. Do not guess.

- [ ] **Step 2: Append the styles**

Append to `filer-task-web/static/style.css`:

```css
.drawer-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: flex-end;
}

.drawer {
  background: var(--panel);
  border-left: 1px solid var(--border);
  width: min(520px, 100%);
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}

.drawer-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.drawer-id {
  font-weight: 600;
}

.drawer-badge {
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 2px 10px;
  font-size: 12px;
  color: var(--muted);
}

.drawer-close {
  margin-left: auto;
  background: none;
  border: none;
  color: var(--muted);
  font-size: 18px;
  cursor: pointer;
}

.drawer-title {
  margin: 8px 0 16px;
  font-size: 16px;
}

.drawer-meta {
  display: grid;
  grid-template-columns: 90px 1fr;
  gap: 4px 12px;
  margin: 0 0 16px;
}

.drawer-meta dt {
  color: var(--muted);
}

.drawer-meta dd {
  margin: 0;
}

.drawer-path {
  overflow-wrap: anywhere;
  color: var(--muted);
}

.drawer-section h4 {
  margin: 16px 0 6px;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--muted);
}

.drawer-section p {
  margin: 0;
  white-space: pre-wrap;
}
```

- [ ] **Step 3: Commit**

The drawer has no caller yet, so there is nothing to see in a browser until Task 6. Commit it as its own reviewable unit.

```bash
rtk git add filer-task-web/static/js/components/TaskDrawer.js filer-task-web/static/style.css
rtk git commit -m "feat(task-web): add a read-only task detail drawer"
```

---

### Task 6: New-task screen

Delivers acceptance criteria 2, 3, 4, and the creation half of 5.

**Files:**
- Create: `filer-task-web/static/js/screens/NewTask.js`
- Modify: `filer-task-web/static/js/app.js:12-24, 26-72`
- Modify: `filer-task-web/static/style.css` (append)

**Interfaces:**
- Consumes: `domainNames`, `prefixesFor`, `tagCatalog`, `taskTypeNames` from `../lib/policy.js`; `fieldError`, `nextNumber`, `preview` from `../lib/newTask.js`; `projectScoped` from `../api/client.js`; `Header` from `../components/Header.js`.
- Produces: `NewTaskScreen({ projectName, onCreated })` — calls `onCreated(showView)` after a successful `POST`.

The request body must match `CreateTaskRequest` (`src/dto.rs:96-107`) exactly: `domain`, `prefix`, `number`, `title`, `type`, `priority`, `milestone` (nullable), `tags` (array, never absent).

- [ ] **Step 1: Write the screen**

Create `filer-task-web/static/js/screens/NewTask.js`:

```js
import { html, useEffect, useMemo, useState } from "../../vendor/preact-htm.js";
import { projectScoped } from "../api/client.js";
import { Header } from "../components/Header.js";
import { fieldError, nextNumber, preview } from "../lib/newTask.js";
import { domainNames, prefixesFor, tagCatalog, taskTypeNames } from "../lib/policy.js";

const PRIORITY_OPTIONS = ["High", "Medium", "Low"];

export function NewTaskScreen({ projectName, onCreated }) {
  const api = useMemo(() => projectScoped(projectName), [projectName]);
  const [policy, setPolicy] = useState(null);
  const [tasks, setTasks] = useState([]);
  const [milestones, setMilestones] = useState([]);
  const [draft, setDraft] = useState(emptyDraft());
  const [numberEdited, setNumberEdited] = useState(false);
  const [rejection, setRejection] = useState(null);
  const [submitting, setSubmitting] = useState(false);

  async function load() {
    try {
      const [loadedPolicy, loadedTasks, loadedMilestones] = await Promise.all([
        api.getPolicy(),
        api.getTasks(),
        api.getMilestones(),
      ]);
      setPolicy(loadedPolicy);
      setTasks(loadedTasks);
      setMilestones(loadedMilestones);
      setRejection(null);
    } catch (err) {
      setRejection(fieldError(err));
    }
  }

  useEffect(() => {
    setDraft(emptyDraft());
    setNumberEdited(false);
    load();
  }, [projectName]);

  const domains = domainNames(policy);
  const prefixes = prefixesFor(policy, draft.domain);
  const types = taskTypeNames(policy);
  const catalog = tagCatalog(policy);

  // The first policy read decides the default domain, prefix and type, and any
  // domain change rescopes the prefix, because a prefix is only legal inside
  // the domain that declares it.
  useEffect(() => {
    if (!policy) {
      return;
    }
    const domain = domains.includes(draft.domain) ? draft.domain : (domains[0] ?? "");
    const domainPrefixes = prefixesFor(policy, domain);
    const prefix = domainPrefixes.includes(draft.prefix) ? draft.prefix : (domainPrefixes[0] ?? "");
    const type = types.includes(draft.type) ? draft.type : (types[0] ?? "");
    if (domain !== draft.domain || prefix !== draft.prefix || type !== draft.type) {
      setDraft((current) => ({ ...current, domain, prefix, type }));
    }
  }, [policy, draft.domain, draft.prefix, draft.type]);

  // The suggested number tracks the domain and prefix until the user overrides
  // it; after that their value stands, so a deliberate id is never overwritten.
  useEffect(() => {
    if (numberEdited || !draft.domain || !draft.prefix) {
      return;
    }
    setDraft((current) => ({ ...current, number: nextNumber(tasks, draft.domain, draft.prefix) }));
  }, [tasks, draft.domain, draft.prefix, numberEdited]);

  const identity = preview(draft.domain, draft.prefix, draft.number, draft.title);

  async function submit(event) {
    event.preventDefault();
    setSubmitting(true);
    try {
      const view = await api.createTask({
        domain: draft.domain,
        prefix: draft.prefix,
        number: draft.number.trim(),
        title: draft.title.trim(),
        type: draft.type,
        priority: draft.priority,
        milestone: draft.milestone || null,
        tags: draft.tags,
      });
      setRejection(null);
      setDraft(emptyDraft());
      setNumberEdited(false);
      await load();
      onCreated(view);
    } catch (err) {
      setRejection(fieldError(err));
    } finally {
      setSubmitting(false);
    }
  }

  function update(field, value) {
    setDraft((current) => ({ ...current, [field]: value }));
  }

  function toggleTag(tag) {
    setDraft((current) => ({
      ...current,
      tags: current.tags.includes(tag)
        ? current.tags.filter((value) => value !== tag)
        : [...current.tags, tag],
    }));
  }

  const ready = Boolean(draft.domain && draft.prefix && draft.number.trim() && draft.title.trim() && draft.type);

  return html`
    <section class="screen">
      <${Header} title="New task" onRefresh=${load} />
      ${rejection && rejection.field === null
        ? html`<p class="screen-error">${rejection.message}</p>`
        : null}
      <form class="new-task-form" onSubmit=${submit}>
        <label>
          Domain
          <select value=${draft.domain} onChange=${(event) => update("domain", event.target.value)}>
            ${domains.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
        </label>
        <label>
          Prefix
          <select value=${draft.prefix} onChange=${(event) => update("prefix", event.target.value)}>
            ${prefixes.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
          <${FieldError} rejection=${rejection} field="prefix" />
        </label>
        <label>
          Number
          <input
            type="text"
            value=${draft.number}
            onInput=${(event) => {
              setNumberEdited(true);
              update("number", event.target.value);
            }}
          />
          <${FieldError} rejection=${rejection} field="number" />
        </label>
        <label>
          Title
          <input type="text" value=${draft.title} onInput=${(event) => update("title", event.target.value)} />
          <${FieldError} rejection=${rejection} field="title" />
        </label>
        <label>
          Type
          <select value=${draft.type} onChange=${(event) => update("type", event.target.value)}>
            ${types.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
          <${FieldError} rejection=${rejection} field="type" />
        </label>
        <label>
          Priority
          <select value=${draft.priority} onChange=${(event) => update("priority", event.target.value)}>
            ${PRIORITY_OPTIONS.map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
        </label>
        <label>
          Milestone
          <select value=${draft.milestone} onChange=${(event) => update("milestone", event.target.value)}>
            <option value="">None</option>
            ${milestoneValues(milestones).map((value) => html`<option key=${value} value=${value}>${value}</option>`)}
          </select>
          <${FieldError} rejection=${rejection} field="milestone" />
        </label>
        <div class="new-task-tags">
          <span class="new-task-tags-label">Tags</span>
          ${catalog === null
            ? html`
                <input
                  type="text"
                  placeholder="comma separated"
                  value=${draft.tags.join(", ")}
                  onInput=${(event) => update("tags", parseTags(event.target.value))}
                />
              `
            : html`
                <div class="chip-row">
                  ${catalog.length === 0
                    ? html`<span class="milestone-empty">This project's tag catalog is empty.</span>`
                    : catalog.map(
                        (tag) => html`
                          <button
                            key=${tag}
                            type="button"
                            class="chip ${draft.tags.includes(tag) ? "chip-active" : ""}"
                            onClick=${() => toggleTag(tag)}
                          >
                            ${tag}
                          </button>
                        `,
                      )}
                </div>
              `}
          <${FieldError} rejection=${rejection} field="tags" />
        </div>
        <p class="new-task-preview">
          ${identity
            ? html`Creates <code>${identity.qualifiedId}</code> at <code>${identity.path}</code>`
            : "Pick a domain, prefix and number to preview the id and path."}
        </p>
        <div class="new-task-actions">
          <button type="submit" disabled=${!ready || submitting}>${submitting ? "Creating…" : "Create task"}</button>
        </div>
      </form>
    </section>
  `;
}

function FieldError({ rejection, field }) {
  if (!rejection || rejection.field !== field) {
    return null;
  }
  return html`
    <span class="field-error">
      ${rejection.message}
      ${rejection.allowed.length > 0 ? html`<span class="field-allowed">Allowed: ${rejection.allowed.join(", ")}</span>` : null}
    </span>
  `;
}

function emptyDraft() {
  return {
    domain: "",
    prefix: "",
    number: "",
    title: "",
    type: "",
    priority: "Medium",
    milestone: "",
    tags: [],
  };
}

function milestoneValues(aggregations) {
  return aggregations.map((entry) => entry.milestone.milestone).filter(Boolean);
}

function parseTags(value) {
  return value
    .split(",")
    .map((tag) => tag.trim())
    .filter(Boolean);
}
```

- [ ] **Step 2: Route the screen and hold the drawer**

In `filer-task-web/static/js/app.js`, add to the imports:

```js
import { NewTaskScreen } from "./screens/NewTask.js";
import { TaskDrawer } from "./components/TaskDrawer.js";
```

Replace the `Screen` component (lines 12-24 as originally written, now carrying the milestones branch from Task 4) with:

```js
function Screen({ screen, project, onCreated }) {
  if (screen === "ready") {
    return html`<${ReadyScreen} projectName=${project.name} />`;
  }
  if (screen === "activity") {
    return html`<${ActivityScreen} projectName=${project.name} />`;
  }
  if (screen === "tasks") {
    return html`<${TasksScreen} projectName=${project.name} />`;
  }
  if (screen === "milestones") {
    return html`<${MilestonesScreen} projectName=${project.name} />`;
  }
  return html`<${NewTaskScreen} projectName=${project.name} onCreated=${onCreated} />`;
}
```

In `App`, add the drawer state next to the other `useState` calls:

```js
  const [drawerView, setDrawerView] = useState(null);
```

and replace the returned shell so the drawer renders above the shell:

```js
  return html`
    <div class="app-shell">
      <${Sidebar}
        screen=${screen}
        onSelectScreen=${setScreen}
        onSwitchProject=${() => setSwitcherOpen(true)}
      />
      <main class="app-main">
        ${project.broken
          ? html`<${BrokenScreen} project=${project} onSwitchProject=${() => setSwitcherOpen(true)} />`
          : html`<${Screen} screen=${screen} project=${project} onCreated=${setDrawerView} />`}
      </main>
      <${TaskDrawer} view=${drawerView} onClose=${() => setDrawerView(null)} />
      ${switcherOpen ? html`<${ProjectSwitcher} onClose=${() => setSwitcherOpen(false)} />` : null}
    </div>
  `;
```

- [ ] **Step 3: Append the styles**

Append to `filer-task-web/static/style.css`:

```css
.new-task-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 480px;
}

.new-task-form label,
.new-task-tags {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--muted);
}

.new-task-form input,
.new-task-form select {
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 6px;
  color: var(--text);
  padding: 6px 8px;
}

.new-task-tags-label {
  font-size: 12px;
}

.field-error {
  color: var(--danger-text);
}

.field-allowed {
  display: block;
  color: var(--muted);
}

.new-task-preview {
  margin: 0;
  color: var(--muted);
  overflow-wrap: anywhere;
}

.new-task-actions {
  display: flex;
  justify-content: flex-end;
}

.new-task-actions button[disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}
```

- [ ] **Step 4: Run the frontend suite**

Run: `rtk cargo test -p filer-task-web --test frontend_js_test`
Expected: PASS. This confirms the lib modules the screen imports still behave; the screen itself is checked in Task 7.

- [ ] **Step 5: Commit**

```bash
rtk git add filer-task-web/static/js/screens/NewTask.js filer-task-web/static/js/app.js filer-task-web/static/style.css
rtk git commit -m "feat(task-web): add the policy-driven new-task form"
```

---

### Task 7: Verify every acceptance criterion and close the task

**Files:**
- Modify: `.tasks/web/WEB-007-build-the-v2-milestones-screen-and-new-task-form.md`

- [ ] **Step 1: Run the full crate suite**

Run: `rtk cargo test -p filer-task-web`
Expected: PASS, all test binaries.

- [ ] **Step 2: Run clippy and formatting**

Run: `rtk cargo clippy -p filer-task-web --all-targets -- -D warnings && rtk cargo fmt --check`
Expected: no output, exit 0. No Rust changed, so this only guards against an accidental edit.

- [ ] **Step 3: Walk the acceptance criteria in a browser**

Run: `rtk cargo run -p filer-task-web -- .` and open the printed address. Check each in order and record what you saw:

1. **Milestones screen.** Click Milestones. Every entry from `GET /api/projects/<project>/milestones` has a card. Each card shows a progress bar whose fill matches its `done`/`total`, its criteria checklist, and its tasks grouped by status in lifecycle order. Compare against `curl` on the endpoint.
2. **Policy-driven options.** Click New task. The domain select lists the project's domains. Switch domains and confirm the prefix select rescopes to that domain's prefixes only. Confirm the type select lists the project's configured types. This repo's `.tasks/config.json` decides whether the tag control is a catalog chip row or a free-text input; confirm it matches the `tags.policy` value the endpoint returns.
3. **Number default and preview.** With domain `web` and prefix `WEB` selected, the number field pre-fills one past the highest existing `WEB-` number, and the preview line reads `Creates web:WEB-0NN at .tasks/web/WEB-0NN-<slug>.md`. Type a title and confirm the slug updates live. Type a number manually and confirm it stops being overwritten when you change nothing else.
4. **Inline rejection.** Set the number back to an id that already exists (for example `007`) and submit. Expected: an inline red message under the **Number** field, no modal, no toast, form values preserved. Then type a prefix not in the domain — you cannot pick an illegal one from the select, so verify this one against the already-passing backend test at `filer-task-web/tests/write_api_test.rs:59-68`, which asserts `code: prefix_not_allowed, field: prefix`, and confirm `FieldError` renders on `field === "prefix"` by inspecting the component. Under a strict tag policy, a rejected tag renders under **Tags** with its allowed list.
5. **Success opens the drawer.** Submit a valid new task. Expected: the drawer slides in from the right showing the created task's qualified id, status `To Do`, its path, and its (empty) criteria heading. Close it. Confirm the task file exists on disk at the previewed path, then `git checkout` or delete the file you created so the repo stays clean.

Stop the server.

- [ ] **Step 4: Delete the scratch task file**

Run: `rtk git status` and remove any `.tasks/**` file created during Step 3.
Expected: `rtk git status` shows only the intended source changes.

- [ ] **Step 5: Review the diff against the constraints**

Run: `rtk git diff main --stat`
Confirm: no file under `filer-task-web/src/` changed, no new dependency, and the total is well under the 700-line guidance for a complex change. If it is not, stop and report rather than continuing.

- [ ] **Step 6: Check the criteria**

Edit `.tasks/web/WEB-007-build-the-v2-milestones-screen-and-new-task-form.md` and change each `- [ ]` to `- [x]` under `## Acceptance Criteria`, but only for criteria Step 3 actually confirmed. If any could not be confirmed, leave it unchecked and report why instead of closing the task.

- [ ] **Step 7: Validate and complete**

```bash
cargo run -q -p filer-task -- validate
cargo run -q -p filer-task -- done web:WEB-007
cargo run -q -p filer-task -- validate
cargo run -q -p filer-task -- show web:WEB-007
```

Expected: validation passes both times, `show` reports status `Done`.

- [ ] **Step 8: Commit**

```bash
rtk git add .tasks/web/WEB-007-build-the-v2-milestones-screen-and-new-task-form.md
rtk git commit -m "chore(tasks): complete WEB-007"
```

---

## Notes for the reviewer

- **WEB-013 unblocks.** It depends on WEB-007, WEB-008, WEB-011, and WEB-012, so it stays blocked; this only removes one of its four blockers.
- **Sidebar counts do not refresh after a creation.** `Sidebar.js:34-62` reloads only when the project changes, so the Tasks count is stale until a project switch or reload. That is pre-existing behavior, not required by any WEB-007 criterion, and fixing it means lifting the counts into a store. Left alone deliberately; worth a follow-up task if it bothers anyone.
- **The path preview duplicates a Rust rule.** `slugify` in `static/js/lib/newTask.js` mirrors `slug` in `filer-task/src/lifecycle.rs:536-546`. There is no way to share it across the language boundary without shipping the path back from a preview endpoint, which the task did not ask for. The module comment says so; if the Rust slug changes, that test file is what catches it.
