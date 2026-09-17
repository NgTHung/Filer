# CORE-039 execution plan

Implement protocol v1 validation and reproducible flat fixtures in an isolated
`filer-core/benchmarks` package. Follow the contracts and conformance matrix in
[the benchmark specification](comparative-performance.md). This plan covers
CORE-039 only. It does not implement a runner or a measured adapter.

## Starting evidence

On 2026-09-17, CORE-039 is In Progress and its dependency CORE-030 is Done.
Taskroot validation passes for 192 tasks with no warnings. The latest commit
starts CORE-039; do not run `start` again when you resume it.

The root workspace contains five crates. `filer-core/benchmarks` does not yet
exist. The existing `filer-core/benches/large_directory.rs` measures public Core
behavior but does not implement this protocol. Keep that benchmark separate.
Use `filer-core/tests/large_directory_paging_test.rs` and
`filer-core/tests/pipeline_filter_contract_test.rs` as test-style references.

An independent Python calculation during planning reproduced every expected
membership, metadata, name-order, viewport, and filter value in the two normative
manifests. This is a planning sanity check, not executable conformance evidence.
The implementation still needs tests for these literals and manifest digests.

## Scope and interfaces

Provide two public interfaces within the benchmark package:

- Protocol validation accepts request bytes, a selected validated manifest,
  declared adapter capabilities, and event lines. It returns stable rejection
  codes or a validated sample with terminal status and structural gate results.
  A run-scoped validator owns accepted sample identities. Per-sample validation
  owns correlation, sequence, action progress, counts, and row consistency.
- Fixture preparation reads a versioned manifest, prepares a new owned fixture
  directory, and verifies filesystem observations against expected rows. Return
  a prepared fixture only after verification succeeds. Provide expected rows
  without filesystem generation so protocol tests do not create 100,000 files.

Keep digest encoding and expected-row construction shared between validation
and fixture preparation. Tests exercise public interfaces from `tests/`.
Record and confirm these test seams before writing tests, following the TDD
skill. The request/event seam is already specified by CORE-030; fixture
preparation is the additional interface proposed here.

Use a small `lib.rs` with private modules for schema parsing, canonical rows and
digests, scenario rules, trace state, manifests, and filesystem preparation.
Split each area further only as its implementation requires. Do not expose
private state transitions or create adapter traits without a current caller.

An isolated Cargo manifest should declare its own workspace and explicit
dependencies, with its own lockfile. Do not add it to the root workspace or
change the root or filer-core dependency declarations. Start with Serde,
serde_json, and SHA-256 support; use tempfile only where fixture ownership or
tests need it. Prefer standard filesystem/time APIs; justify any additional
dependency with an actual portability requirement. Add local ignores for
generated build and fixture artifacts.

## Commit 1: settle protocol interpretations

Refine the existing CORE-039 task and the relevant specification paragraphs
before production code. Preserve protocol v1 meanings and the checked CORE-030
history. Record the following decisions with worked traces or table entries:

1. Define each scenario's exact action sequence, allowed optional phases,
   output scopes, continuation values, and request/cache restrictions. Explicitly
   include `open` before `page-0002` through `page-0040` for `browse.next`, and
   place its final listing proof. Specify the journey's initial request settings
   and action-specific sort/filter changes without adding wire fields.
2. Clarify counts for continuation actions. Counts reset at each action while
   the final listing contains the whole chain. Give a page-0040 example showing
   its 16-row page, 10,000-row listing proof, action counts, and sample totals.
   Repeated proof rows must not accidentally count as new emitted rows.
3. Reconcile lexical errors with stable result codes. The matrix's version
   rule and the malformed request example overlap; retain the example's required
   `invalid_schema` result and define the code for an otherwise valid request
   with a numeric-string version. Specify codes for unknown phases, malformed
   row fields, non-event JSON, and plain stdout diagnostics. Do not let Serde
   error wording become the public rejection contract.
4. State which checks remain mandatory on non-success traces. Correlation,
   framing, sequence, row validity, and terminal integrity still apply; missing
   success milestones alone must not reject a legitimate capability result.
   Define capability failure precedence for the normative unsupported-success
   example, which also lacks success phases.
5. Make the sparse-filter matrix ownership explicit. CORE-039 proves that the
   unfiltered streaming cap is not used for supported filtered/snapshot work.
   CORE-042 supplies the sparse fixture and its end-to-end continuation proof.
   Do not add an unsupported scenario to v1 to satisfy that future case.

Also define how trusted capability declarations and requested metric names
enter the validator. They are caller context, not new request fields. Profile
collection, profile persistence, cache preparation, and real ready barriers
remain runner responsibilities. Wire validation checks their defined references.

Validate task metadata and commit these clarifications before implementation.
If an ambiguity requires changing a v1 meaning rather than clarifying it, record
that decision explicitly before proceeding with dependent code.

## Commit 2: isolated package, strict schema, and golden messages

Capture root workspace metadata, the root lockfile, and the filer-core normal,
build, and development dependency trees before creating the package.

Write one failing public-interface test at a time, implement only enough to pass
it, and continue through the schema cases. Establish:

- One complete UTF-8 JSON object per newline-terminated event; request framing
  accepts exactly one object. Detect truncated input and unexpected stdout.
- Unknown, missing, and duplicate keys are rejected at every strict object.
  Do not first deserialize into a map that discards duplicate keys. Metric maps
  allow metric names but still reject duplicate names and invalid value shapes.
- Exact integer types and ranges, identifiers, digest syntax, closed tags,
  requested-field order and uniqueness, and valid unavailable-value reasons.
- Request/event projection types and a typed error with stable protocol code
  and useful location context, such as line, sequence, action, or field.

Check in the specification's valid request, complete `not_supported` trace,
invalid request, and invalid page event under `tests/golden/`. Keep the deliberate
one-row/256-count mismatch only in its negative case. Add isolated mutations
for the other schema failures instead of making one input test several errors.

Exit evidence: focused schema tests pass; nested metadata proves isolation;
root manifests, lockfile, and dependency trees are unchanged. Commit the package,
golden messages, and tests together.

## Commit 3: canonical digests and manifest validation

Copy the two normative manifests into `fixtures/manifests/`. Keep generated
filesystem entries out of Git. Implement their strict parsing and validate
schema version, generator parameters, metadata projection, fixture identity,
expected values, and manifest digest.

Add behavior tests through manifest loading and output validation for:

- Length-prefixed UTF-8 token encoding, signed timestamps, integer formatting,
  null directory sizes, and the documented one-row page digest.
- Identity-byte sorting for membership/metadata scopes and preserved order for
  ordered/page/viewport scopes. Verify reordered provider membership passes,
  while reordered named output fails its expected digest.
- Exact generated identities, kinds, sizes, and timestamps at representative
  indices, including hidden files and directories and extension changes.
- Both complete row sets against the checked-in expected digests, including
  90 filtered rows and the 40-row viewports for flat-10k.
- Manifest record order and digest coverage exactly as specified. Independently
  validate every expected journey value, including values not covered by the
  manifest digest's listed records. Reject altered expectations or parameters.

Use fixed specification values as test oracles. A helper may construct complete
trace payloads, but it must not generate both the expected hash and the assertion
target through the same production function. Retain the small fixed golden
messages to catch serializer/parser agreement on an incorrect format.

Exit evidence: both manifests validate and all digest mutation tests fail with
the intended error. No filesystem fixture generation is needed in this commit.

## Commit 4: trace lifecycle and browse correctness

Build incremental event ingestion with explicit finalization at EOF. Keep the
run identity registry separate from a sample's state. Test acceptance and
rejection through the same public interface the later runner will use.

Implement correlation, sequence from zero, nondecreasing timestamps, legal
sample/action transitions, singleton phases, required completion, action count
resets, and sample totals. Reject events after terminal completion and require
explicit EOF finalization before returning a complete validated sample.

Validate rows against the manifest, including kind for fast listings and exact
metadata when requested. Check output scope, row count, digest, continuation,
and scenario size. Distinguish permitted repetition across a viewport, page,
and full listing proof from duplicate rows inside an event or across pages of
one continuation chain. Relate visible prefixes to the observed page/listing
sequence; membership equality alone cannot prove that a viewport was correct.

Add accepting full traces for `browse.fast.first`, `browse.fast.scale`,
`browse.metadata.first`, and `browse.next`. Every claimed output must contain
its full rows. For continuation, prove all 40 pages, 256 rows on pages 1 to 39,
16 rows on page 40, correct `more`/`end`, and complete membership. Include two
different valid provider enumeration orders.

Mutate valid traces to exercise missing rows, duplicate identities, wrong
digests, wrong counts, sequence gaps, clock regression, stale actions, missing
phases, duplicate phases, and terminal failures. Preserve the specification's
exact test names and error codes in the tests or their parameterized cases.

Exit evidence: all four browse traces pass, targeted invalid traces return
stable errors, and valid non-success results remain distinct from success.

## Commit 5: transforms, reference journey, and gate classification

Extend shared scenario rules and trace validation for `view.sort.name`,
`view.filter.common`, `browse.refresh`, and `journey.browse-reference`.

Prove full name order and first viewport, exactly 90 ordered filtered rows,
clear-filter restoration, and the refresh listing/transform/view sequence.
Validate the journey's entire action order and reject skipped pages, output
from completed actions, and commits after their action finishes. Reset page
uniqueness for a new enumeration while retaining sample/action correlation.

Classify the first-page gate from declared capability and observable counts:
streaming unfiltered provider-order work with examined counts of 512 passes;
513 fails; unavailable counts produce `not_evaluable`. Test non-streaming,
filtered, and snapshot-only exclusions. Keep semantic validity separate from
structural gate results. Unavailable resource metrics are neither zero nor a
semantic failure; unavailable correctness-required counts remain failures.

Test unsupported declared capabilities reported as success against the
normative `unsupported_reported_as_success` code. Preserve `not_supported`,
`error`, and `cancelled` diagnostics and exclude them from timing eligibility.

Trace tests prove the ordering of declared barriers and milestones. They cannot
prove that a real adapter performed no early work, collected no extra metadata,
used one actual monotonic clock, or started a new OS enumeration. Those runtime
proofs belong to CORE-040/CORE-032; document that limit in the package README.

Exit evidence: all eight v1 accepting scenario tests and all 24 rejection cases
in the specification have executable coverage, plus applicable gate tests.

## Commit 6: filesystem fixture generation and readback

After protocol validation lands, implement filesystem preparation from the
validated manifests. Keep generation and readback outside event timestamps
and timed-sample code. CORE-040 will call this interface before `sample.started`.

Create a new owned fixture directory, generate all entries, set file lengths
without writing their full logical contents, then apply final modification
times to files and directories. Return errors with operation/path context.
Do not overwrite unrelated existing data or report a partially created fixture
as ready. Make cleanup ownership and cleanup failure reporting explicit.

Read actual filesystem metadata and compare it with expected relative rows.
Reject missing/extra entries, wrong kinds, changed sizes, and changed timestamps.
Check timestamp representability on the host filesystem; do not round away a
manifest mismatch. Avoid an extra timestamp library unless platform evidence
requires it. Do not include root path, inode, allocation, access time, ownership,
or enumeration position in identity.

Tests must generate and verify both flat-10k and flat-100k on disk. Compare
separate preparation roots to prove root-independent results. Use sparse file
creation to keep physical storage practical, and report preparation failure
clearly on unsuitable filesystems. Exercise corrupt manifests before creation,
existing-target conflicts, readback corruption, and observable I/O failures.
Keep ordinary schema tests independent of expensive filesystem setup. The final
acceptance command must run the real 100k test even if it has a dedicated target.

Exit evidence: both real fixtures match membership and all requested metadata;
failed preparation cannot be passed to the future runner as a ready fixture.

## Commit 7: verification, evidence, and handoff

Document package commands, the two public interfaces, ownership of setup and
cleanup, conformance coverage, and the limits of synthetic traces. Record
relevant platform/filesystem details for the real fixture test run.

Run focused test targets as each slice lands, then run these final checks from
the repository root:

```bash
cargo fmt --manifest-path filer-core/benchmarks/Cargo.toml --all -- --check
cargo check --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo clippy --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path filer-core/benchmarks/Cargo.toml --locked --all-targets
cargo test --manifest-path filer-core/benchmarks/Cargo.toml --locked --doc
cargo metadata --manifest-path filer-core/benchmarks/Cargo.toml --locked --no-deps --format-version 1
cargo metadata --locked --no-deps --format-version 1
cargo tree --locked -p filer-core --edges normal,build,dev
git diff --check
cargo run -q -p taskroot -- validate
```

Compare root workspace members, dependency-tree output, and root manifest and
lockfile content with the captured baseline. The nested package must not enter
the normal development graph. Full Core feature-matrix execution is unnecessary
if Core code, features, dependency graphs, and existing benchmarks are unchanged;
if any change, revisit scope and run the affected feature checks.

Map the evidence to all three acceptance criteria in CORE-039. Check criteria
only after their implementation and tests pass, validate, run
`cargo run -q -p taskroot -- done core:CORE-039`, validate again, inspect
`cargo run -q -p taskroot -- show core:CORE-039`, and run
`cargo run -q -p taskroot -- list`. Commit the final documentation and task state.
CORE-040 can then consume this package; this plan does not start that task.

## Change-size and execution rules

Each numbered stage is a separate green commit. Use vertical TDD slices within
each code commit; do not write all tests first and all implementation afterward.
Keep complex diffs below 700 changed lines and other non-mechanical diffs below
1,000. Target Rust modules below 700 lines, with tests under `tests/`.

The package, 24 rejection cases, eight scenario traces, and real fixture support
will likely exceed the total-change guidance. These stages follow actual
contract dependencies: schema before digests, digests before semantic trace
checks, and validated manifests before filesystem generation. There is no
implementation diff yet, so line estimates are not evidence. Inspect the real
diff before each commit and split an oversized stage into smaller tested
behaviors. The first executable stage to land is strict schema validation with
golden messages; fixture generation remains a later separate commit.

Use `Result` and explicit error handling in production code. Avoid unnecessary
clones, new dependencies, hidden error suppression, and inline test modules.
Preserve unrelated work in the checkout when staging commits.
