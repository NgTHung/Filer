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
