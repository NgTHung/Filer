import assert from "node:assert/strict";
import { test } from "node:test";

import {
  addDomainOperation,
  addPrefixOperation,
  addTagOperation,
  addTaskTypeOperation,
  removeDomainOperation,
  removePrefixOperation,
  removeTagOperation,
  removeTaskTypeOperation,
  sectionForOperation,
  tagFlipWarning,
} from "../../static/js/lib/policyOps.js";

test("a domain carries its initial prefix list", () => {
  assert.deepEqual(addDomainOperation("web", "WEB, UTILS ,"), {
    operation: "add_domain",
    name: "web",
    prefixes: ["WEB", "UTILS"],
  });
});

test("a domain with no usable prefix is not a request", () => {
  assert.equal(addDomainOperation("web", ""), null);
  assert.equal(addDomainOperation("web", " , , "), null);
});

test("the remaining builders emit their operation and operands", () => {
  assert.deepEqual(removeDomainOperation("web"), { operation: "remove_domain", name: "web" });
  assert.deepEqual(addPrefixOperation("web", "WEB"), {
    operation: "add_prefix",
    domain: "web",
    prefix: "WEB",
  });
  assert.deepEqual(removePrefixOperation("web", "WEB"), {
    operation: "remove_prefix",
    domain: "web",
    prefix: "WEB",
  });
  assert.deepEqual(removeTaskTypeOperation("Feature"), {
    operation: "remove_task_type",
    name: "Feature",
  });
  assert.deepEqual(addTagOperation("web"), { operation: "add_tag", tag: "web" });
  assert.deepEqual(removeTagOperation("web"), { operation: "remove_tag", tag: "web" });
});

test("every operand is trimmed", () => {
  assert.deepEqual(addPrefixOperation("  web ", "  WEB "), {
    operation: "add_prefix",
    domain: "web",
    prefix: "WEB",
  });
  assert.deepEqual(addTagOperation("  web  "), { operation: "add_tag", tag: "web" });
  assert.deepEqual(addDomainOperation("  web  ", "WEB"), {
    operation: "add_domain",
    name: "web",
    prefixes: ["WEB"],
  });
});

test("a blank required operand yields no request at all", () => {
  assert.equal(addDomainOperation("   ", "WEB"), null);
  assert.equal(removeDomainOperation("  "), null);
  assert.equal(addPrefixOperation("web", "  "), null);
  assert.equal(addPrefixOperation("  ", "WEB"), null);
  assert.equal(removePrefixOperation("web", ""), null);
  assert.equal(addTaskTypeOperation("  ", "acceptance", false), null);
  assert.equal(removeTaskTypeOperation(""), null);
  assert.equal(addTagOperation("  "), null);
  assert.equal(removeTagOperation(""), null);
});

test("a task type omits its role unless it is the milestone one", () => {
  assert.deepEqual(addTaskTypeOperation("Chore", "acceptance", false), {
    operation: "add_task_type",
    name: "Chore",
    criteria: "acceptance",
  });
  assert.deepEqual(addTaskTypeOperation("ReleaseGate", "exit", true), {
    operation: "add_task_type",
    name: "ReleaseGate",
    criteria: "exit",
    role: "milestone",
  });
});

test("a task type falls back to acceptance criteria", () => {
  assert.deepEqual(addTaskTypeOperation("Chore", "", false), {
    operation: "add_task_type",
    name: "Chore",
    criteria: "acceptance",
  });
});

test("each operation reports the section its refusal belongs to", () => {
  assert.equal(sectionForOperation(addDomainOperation("web", "WEB")), "domains");
  assert.equal(sectionForOperation(removeDomainOperation("web")), "domains");
  assert.equal(sectionForOperation(addPrefixOperation("web", "WEB")), "domain:web");
  assert.equal(sectionForOperation(removePrefixOperation("core", "CORE")), "domain:core");
  assert.equal(sectionForOperation(addTaskTypeOperation("Chore", "acceptance", false)), "task_types");
  assert.equal(sectionForOperation(removeTaskTypeOperation("Chore")), "task_types");
  assert.equal(sectionForOperation(addTagOperation("web")), "tags");
  assert.equal(sectionForOperation(removeTagOperation("web")), "tags");
});

test("only an open tag policy warns that the catalog is about to close", () => {
  assert.equal(typeof tagFlipWarning({ tags: { policy: "open" } }), "string");
  assert.equal(tagFlipWarning({ tags: { policy: "strict", allowed: [] } }), null);
  assert.equal(tagFlipWarning({ tags: { policy: "strict", allowed: ["web"] } }), null);
  assert.equal(tagFlipWarning(null), null);
});
