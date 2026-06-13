# Documentation and Comment-Rule Compliance Review (CORE-012)

Do the filer-core module docs follow the AGENTS.md "explain WHY, not WHAT" rule, do inline
comments avoid the banned patterns (placeholders, section dividers, markdown, restating code),
and do the README files still match the code? This report flags comment-rule violations with
`file:line`, lists README claims that have drifted from the code, and proposes doc-fix
candidates for the CORE-013 backlog. It is review-only and changes no production code or docs.

Evidence is cited as `path:line` against the crate at the time of review. Test modules under
`src/tests/` are out of scope. There is no `DESIGN.md` for filer-core; the only `DESIGN.md` in
the repo lives under `filer-ecosystem/`, which is out of the current core focus. README scope
here is the root `README.md`, `filer-core/README.md`, and `docs/README.md`.

## Headline: a command-rename sweep corrupted README prose

The single largest documentation defect is not a missing doc. It is text damage. A
find-and-replace pass that renamed commands to their `*Compat` variants ran across the README
prose and replaced ordinary English words with internal type names. Sentences that should read
"scan, search, preview" now read "ScanPathCompat, SearchNodeCompat, preview".

Root `README.md` has six such hits:

- `README.md:11` architecture comment: "Core library (actors, VFS, SearchNodeCompat, preview)"
  should say "search".
- `README.md:29` "dependable navigation, scanning, SearchNodeCompat, file operations" should
  say "search".
- `README.md:41` "stale-event guards are now in place for ScanPathCompat, SearchNodeCompat,
  preview, refresh" should say "scan, search".
- `README.md:59` "navigation, ScanPathCompat, SearchNodeCompat, preview, metadata" should say
  "scan, search".
- `README.md:72` "Location-native read, WatchNodeCompat, write, preview" should say "watch".

`filer-core/README.md` carries 36 `Compat` occurrences. Many are legitimate: the "NodeId
Surfaces", "Command API", and "Rust Migration" tables (`filer-core/README.md:209-251`) name
real command variants and should keep them. The damage is in the prose that describes flows and
capabilities, where a generic verb was clobbered:

- `filer-core/README.md:43` "request ids and stale-result guards for ScanPathCompat,
  SearchNodeCompat, preview, metadata, and refresh flows" should read "scan, search".
- `filer-core/README.md:45` "operation ids for CopyNodeCompat, MoveNodeCompat, DeleteNodeCompat,
  RenameNodeCompat, create file" should read "copy, move, delete, rename".
- `filer-core/README.md:53` "Location-aware navigation, ScanPathCompat, SearchNodeCompat,
  preview, metadata" should read "scan, search".
- `filer-core/README.md:56` "watcher changes, operation completion" prose nearby is fine, but
  `:127` "directory MoveNodeCompat, DeleteNodeCompat, and RenameNodeCompat also invalidate the
  old subtree" should read "move, delete, and rename".
- `filer-core/README.md:141` "navigation-driven scans, refresh, SearchNodeCompat, preview" and
  "CopyNodeCompat, MoveNodeCompat, DeleteNodeCompat, RenameNodeCompat, create file" should read
  the plain verbs.
- `filer-core/README.md:282` "ScanPathCompat progress is request-scoped" should read "scan".

The fix is mechanical but needs a human eye per line: keep `*Compat` only where the sentence
genuinely names a command variant, and restore the English word everywhere else. A blind
reverse-replace would re-break the tables. Severity: Medium. The READMEs are the first thing a
contributor or frontend author reads, and right now they read as machine noise.

## filer-core README "Modules" table is structurally stale

`filer-core/README.md:23-34` lists the crate layout. It does not match `src/`:

- It lists `bus/` for "Message routing". There is no `bus/` directory
  (`ls filer-core/src` shows `actors api model modules pipeline services tests utils vfs`).
  Routing lives in `actors/router.rs`.
- It attributes "Scanner, searcher, watcher, previewer, and operation workers" to `actors/`.
  Those workers live under `modules/` today: `modules/scan/scanner.rs`,
  `modules/search/searcher.rs`, `modules/watch/watcher.rs`,
  `modules/preview/previewer.rs`, `modules/operations/operator.rs`. `actors/` holds the actor
  infrastructure (`actors/mod.rs`, `actors/router.rs`, `actors/cancel.rs`), not the workers.
- It omits `modules/` entirely, which is now the largest functional subtree.

A contributor using this table to find the scanner would look in the wrong place. Severity:
Medium. The table should list `actors/` (infrastructure), `modules/` (the per-feature workers),
and drop `bus/`.

## docs/README.md is empty

`docs/README.md` exists but has zero content (one empty line). Either it is a forgotten
placeholder or an index that was never written. `docs/` now holds the review reports under
`docs/reviews/filer-core/` and `docs/task-tracking.md`, so an index would help. Severity: Low.
Decide whether to fill it as a `docs/` index or delete it.

## Comment-rule violations

### Stale TODO/placeholder comments where the code is already done

The AGENTS rule bans placeholder comments ("for now", "TODO: extract this later"). Three sites
are not just style nits; the comment actively contradicts the code beside it.

- `vfs/local.rs:214` opens a `# TODO` rustdoc section on `read_header` that lists implementation
  steps ("Open file with `tokio::fs::File::open`", "Allocate `buf`", "`file.read_exact`").
  The function below (`vfs/local.rs:220-229`) already implements exactly those steps. The TODO
  is done. A reader sees a `# TODO` heading in the published docs for a finished method and
  assumes the method is a stub. Delete the section or replace it with a one-line WHY note.
  Severity: Medium (it is in rendered rustdoc, not just a line comment).
- `pipeline/mod.rs:158` says "TODO: Add more filter stages as implemented" and lists
  "exclude_extensions" on `:159`, but `:160-165` implement `exclude_extensions` right under it.
  Only `min_size / max_size` (`:167`) and `name_pattern` (`:168`) remain. The TODO is half
  stale and misleads about what is built. Severity: Low.
- `pipeline/mod.rs:188` "// Map to extension for now" is a "for now" placeholder on the
  `GroupBy::Type` arm. It also hides a real behavior note: grouping by type silently falls back
  to grouping by extension. Rephrase as a WHY comment that states the limitation without "for
  now". Severity: Low.

`services/metadata/extractors/document.rs:15` ("add a dedicated crate to fill those fields
later") and `:66` ("add a dedicated crate (e.g. docx-rs) to fill fields") use "later" but are
honest descriptions of a real limitation, not stubs of finished code. They are acceptable as-is
or could drop the word "later". Severity: Low, optional.

`model/operation.rs:30` and `model/request.rs:19` describe `DEFAULT` consts as "compatibility
placeholders". That is the documented meaning of the constant, not a placeholder comment. Not a
violation.

### Inline comments that restate the code

`model/node.rs` is the worst offender for WHAT-not-WHY. `FileNode::from_path` and
`from_dir_entry` are narrated step by step with comments that add nothing the code does not say:

- `node.rs:95` "// Get metadata", `:99` "// Extract file name", `:106` "// Generate ID",
  `:112` "// Determine kind", `:128` "// Get times", `:133` "// Get size",
  `:136` "// Determine if hidden (Unix: starts with dot)", `:146` "// Get permissions".
- The same labels repeat in the second constructor: `:183`, `:190`, `:196`, `:212`, `:217`,
  `:220`, `:230`.

These restate the immediately following line. The rule says delete comments that restate
obvious code. The one with content worth keeping is the "Unix: starts with dot" note, which can
shrink to the rule it encodes. Severity: Low, but it is the clearest concentration of the
violation in the crate.

Lighter cases of the same pattern: `pipeline/mod.rs:145` "// Add filter stages", `:171`
"// Add sort stage", `:180` "// Add group stage"; `pipeline/group.rs:56` "// Convert to ordered
Vec", `:67` "// Sort groups by label"; `model/query.rs:106` "// Validate regex compiles at
parse time" (borderline, states intent). These restate structure. Severity: Low.

### Markdown emphasis in doc comments

The rule lists "`**bold**`, `_italic_` in code comments" under never-use. Doc comments
(`///`, `//!`) are rendered by rustdoc, where markdown is the native format, so this is a
genuine tension between the literal rule and rustdoc convention. Sites using `**bold**`:
`api/module.rs:21`, `:23`, `:24`; `api/handle.rs:57-60`; `model/session.rs:12`, `:13`;
`vfs/provider.rs:15`, `:16`, `:164`, `:165`; `services/preview/registry.rs:59`, `:64`, `:68`;
`services/mime/detector.rs:345`.

Most are bolded lead-ins on list items in module docs, which read well in rendered docs. Flag
for a project decision rather than a blanket fix: either carve out an explicit exception for
rustdoc-rendered doc comments in AGENTS.md, or convert the bold lead-ins to plain text. Treat
plain `//` inline comments as the hard no-markdown zone. Severity: Low, needs a rule decision.

One of these is also a meta-comment: `services/mime/detector.rs:345` "This is the **only**
non-todo function in this file." References a "todo" state of the file and editorializes. Drop
or rewrite as a plain statement of what the function is. Severity: Low.

### Section-divider comments

None found. The grep for `// ====`, `// ----`, and `// ####` divider patterns returned no
matches in production code. The ban is respected. The closest is `lib.rs:12-50`
("// Re-exports", "// Services", "// VFS providers", "// Actor infrastructure", "// Pipeline
types"), which are short re-export group labels, not ASCII dividers. Acceptable.

## Module-doc coverage and quality

Seventeen production files carry `//!` module docs; the crate has roughly forty production
source files, so over half have none. The rule asks for a module doc with a `#` title, plain
language, design rationale, and a runnable example when adding functionality. Notable files
with substantial public surface and no `//!` doc: `errors.rs`, `model/node.rs`, `vfs/local.rs`,
`vfs/provider.rs`, `pipeline/filter.rs`, `pipeline/sort.rs`, `pipeline/group.rs`, and the
`utils/` and `services/metadata/extractors/` files.

This is not a per-line violation, since the rule ties module docs to adding functionality, not
to a hard "every file" mandate. But the gap matters for a crate this central. The files that do
have module docs (`api/module.rs`, `modules/navigation/mod.rs`, `model/session.rs`) follow the
WHY-in-prose style well and are a good template. Severity: Low. Track as coverage debt, not a
blocker.

## Follow-up doc-fix candidates

These are candidates for the CORE-013 remediation backlog, not new tasks created here.

- Repair the `*Compat` prose corruption in `README.md` (5 sites) and `filer-core/README.md`
  (prose sites, not the command tables). Restore plain verbs; keep `*Compat` only where a real
  command variant is named. Severity: Medium. Highest-value doc fix.
- Rewrite the `filer-core/README.md` "Modules" table: drop `bus/`, add `modules/`, and move the
  workers from `actors/` to `modules/`. Severity: Medium.
- Delete or rewrite the stale `# TODO` rustdoc section on `vfs/local.rs:214`; the method is
  implemented. Severity: Medium.
- Trim the stale TODO at `pipeline/mod.rs:158` and the "for now" placeholder at `:188`.
  Severity: Low.
- Strip the WHAT-restating comments in `model/node.rs` (the "// Get X" / "// Determine X"
  block), keeping only the hidden-file rule note. Severity: Low.
- Decide and document the rustdoc markdown exception in AGENTS.md, then either keep or remove
  the `**bold**` lead-ins listed above; remove the "non-todo function" meta-comment at
  `detector.rs:345`. Severity: Low, needs a rule decision.
- Decide the fate of empty `docs/README.md`: fill as a `docs/` index or delete. Severity: Low.
- Backfill `//!` module docs for the high-traffic doc-less files (`errors.rs`, `model/node.rs`,
  `vfs/provider.rs`, `pipeline/filter.rs`/`sort.rs`/`group.rs`) as those modules are next
  touched. Severity: Low. Coverage debt.
