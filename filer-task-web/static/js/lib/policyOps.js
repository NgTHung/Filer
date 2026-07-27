// Builds the one-operation bodies PATCH /policy accepts.
//
// A builder returns null when a required operand is blank, so a form can gate
// its submit button on the request itself rather than repeating the rules, and
// no half-formed request ever reaches the server.

import { splitList } from "./text.js";

export function addDomainOperation(name, prefixesText) {
  const domain = name.trim();
  const prefixes = splitList(prefixesText);
  if (!domain || prefixes.length === 0) {
    return null;
  }
  return { operation: "add_domain", name: domain, prefixes };
}

export function removeDomainOperation(name) {
  const domain = name.trim();
  return domain ? { operation: "remove_domain", name: domain } : null;
}

export function addPrefixOperation(domain, prefix) {
  return prefixOperation("add_prefix", domain, prefix);
}

export function removePrefixOperation(domain, prefix) {
  return prefixOperation("remove_prefix", domain, prefix);
}

// The server takes at most one milestone-role type, and rejects an empty role
// outright, so the key is absent rather than blank when the role is not wanted.
export function addTaskTypeOperation(name, criteria, isMilestone) {
  const type = name.trim();
  if (!type) {
    return null;
  }
  const operation = {
    operation: "add_task_type",
    name: type,
    criteria: criteria === "exit" ? "exit" : "acceptance",
  };
  return isMilestone ? { ...operation, role: "milestone" } : operation;
}

export function removeTaskTypeOperation(name) {
  const type = name.trim();
  return type ? { operation: "remove_task_type", name: type } : null;
}

export function addTagOperation(tag) {
  return tagOperation("add_tag", tag);
}

export function removeTagOperation(tag) {
  return tagOperation("remove_tag", tag);
}

// Prefix refusals are scoped to their own domain so one bad prefix cannot paint
// an error across every other domain row.
export function sectionForOperation(operation) {
  switch (operation.operation) {
    case "add_prefix":
    case "remove_prefix":
      return `domain:${operation.domain}`;
    case "add_task_type":
    case "remove_task_type":
      return "task_types";
    case "add_tag":
    case "remove_tag":
      return "tags";
    default:
      return "domains";
  }
}

// Adding the first tag converts an open policy into a strict catalog holding
// only that tag, which rejects every task already carrying a different one.
export function tagFlipWarning(policy) {
  if (policy?.tags?.policy !== "open") {
    return null;
  }
  return "This project accepts any tag. Adding one closes the catalog to that tag alone, which rejects tasks already carrying another.";
}

function prefixOperation(operation, domain, prefix) {
  const scope = domain.trim();
  const value = prefix.trim();
  if (!scope || !value) {
    return null;
  }
  return { operation, domain: scope, prefix: value };
}

function tagOperation(operation, tag) {
  const value = tag.trim();
  return value ? { operation, tag: value } : null;
}
