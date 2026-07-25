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
