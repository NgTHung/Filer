I. Reliability & Known Debt (Ship-blockers)

Q1. When do bugs beat features?
BUGS.md lists 8+ known user-visible issues — context menu misplacement, unstable image rendering, broken status bar, session ID not randomized, search view corruption on clear, grouping with missing lazy-loaded data. Meanwhile the core ROADMAP has 0/8 "Reliability First" items checked. Is there a rule for when a known bug must be fixed before any new feature lands? If not, what determines prioritization today?
The current BUGS.md is bug tracker for UI. After considerasion, we have come to a conclusion to postpone our UI(filer-app) until the filer-core tailored as mentions by the updated docs. So we will prioritize the bug fixes for filer-core but for filer-app we wont fix it unless stated otherwise.

Q2. What does the test suite actually prove?
There are 27 test files but the Reliability section explicitly calls out missing watcher regression tests, cache invalidation tests, stale-event guards, and cancellation tests as High priority gaps. The session ID bug is a correctness failure that tests should have caught. Is the current test coverage protecting against regressions, or mostly validating the happy path?
This project is TDD, everything is test driven, so it not only "happy path" but serve a better purpose, but we will make sure that the test are reliable.

Q3. Is "Restore previous folder on app start" actually MVP?
It's marked MVP and unchecked. Every restart resets state — no persistent path, sort, group, preview panel, or layout. An Explorer replacement that forgets where you were is a daily friction point. How long has this been unchecked, and what is blocking it?
It one of the reason we running these question, to shaping the future direction of this project, to achive a consistant work/path for this project.
---
II. Scope & Over-Engineering

Q4. What is the purpose of the crypto module right now?
There are 95 todo!() calls across crypto/cipher.rs, crypto/key.rs, and crypto/vault.rs. No roadmap item explains what user story or product requirement drove building these abstractions. There is no "encrypted vault" in the app roadmap. Is this genuine planned work or speculative infrastructure that's adding surface area without delivering value?
After consideration this is indeed overengineer, so we've made plan to remove it.

Q5. Do the 6 skeleton providers earn their place today?
S3, WebDAV, FTP, FUSE, Kubernetes, and archive providers are structure without implementation — roughly 600+ lines of stubs, each 10-13 todo!() calls deep, none wired into the session/preview/operation pipeline. These create the appearance of multi-provider support without the reality. At what point does keeping these stubs become a maintenance liability vs. a useful scaffold? Could they be removed until there's a working implementation?
We currently just after MVP, these feature are a bit overkill for a MVP so our next move is to fill in these tech debt.

Q6. Is the ecosystem contract layer built too far ahead of the runtime?
filer-ecosystem has a complete extension manifest system, permission model, profile operation semantics, and package format validation — but no WASM runtime, no native plugin loader, no extension manager UI, no pack/unpack CLI, and no wire-safe extension envelope in core. The contract exists for a runtime that doesn't. What signals would tell you that the contract stabilized correctly without a real extension to stress-test it?
filer-ecosystem is a needed piece and we have agreed on simplified it and postpone on implementing it but we still need to propose it for the future compatabilities, it might seen as an overengineer take but i believe this is needed.

Q7. Where does "programmer-oriented file manager" end and "IDE" begin?
The roadmaps include: integrated terminal, git panel (status/diff/branch/commit/stash/push/pull), SSH/SFTP browsing, S3 provider, project detection, syntax-aware metadata, task/script launcher, file converters, and lightweight diagnostics from external tools. Each item individually is reasonable. Together, is this still a "file manager with programmer support" or a slow drift toward a full IDE? Who is the primary user — someone who wants a better Explorer, or a developer who wants a VS Code sidebar?
Filer is designed with file manager in mind, but after seeing the potential of it, we have making it boarder than our original vision. For the seperation, we dont want it to replace compiler, editor, it should able to handle some small feature such as git, terminal, converter and so on, which is everyday utilities for developer but not fully blown to replace any crutial tools such as vscode/ide/browser.

---
III. Architecture Health

Q8. When does the Arc<dyn Any> extension seam become a problem?
Command::Extension uses Arc<dyn Any> for payloads. This is explicitly listed as High-priority technical debt. It works for trusted in-process modules but is unsuitable for the ecosystem layer. Today there appear to be zero third-party or even first-party extensions actually using this seam. Before the ecosystem system is considered usable, this needs replacing with serializable envelopes. Is there a concrete plan or milestone for this change?
This will be design after the proposal of extension system(not implementation).

Q9. Is the multi-frontend architecture earning its complexity today?
The architecture is designed for desktop (Iced) + future web client + server transport. The wire protocol roadmap (serde for all public types, versioned envelopes, WebSocket sessions) is zero percent complete. Meanwhile the single Iced desktop app has known bugs. How much of the current complexity — sessions as isolation boundaries, actor model, FsProvider abstraction — is justified by the current one-frontend reality, and how much is pre-investing for clients that may never exist?
It will soon payoff since we have confirm it posibilities, our next step is to fill in these tech dept and remove overengineered feature like crypto/kubernetes and so on.

Q10. Does the preview system's local-path assumption need fixing before remote providers?
The preview registry still makes local-path assumptions for magic-byte fallback and provider-backed generation. This means a future S3 or WebDAV provider cannot preview files through pure FsProvider without special-casing. The ROADMAP lists this as High-priority debt. If remote providers require the preview system to work correctly, does fixing preview need to precede any remote provider implementation?
Yes, as a matter of fact, We planned a parsing system which address this problem.
---
IV. Product & Delivery

Q11. What is the minimum bar for a public v0.1?
There is no version number, no release, no distribution package (.msi, .exe, .appx), and no download link. The app runs via cargo run. What set of checked boxes on the roadmap would constitute "good enough to share with someone who doesn't write Rust"? Is the project building toward a release, or primarily toward architectural completeness?
This is a difficult decision, I have a plan to propose: MVP should be v0.1, or something similar, we currently on our way to decide what should be our Version schema.

Q12. How does large-directory performance fit into "Explorer replacement"?
Large-directory virtualization is listed as "Polish" in the app roadmap and "Medium" technical debt in core. But Windows Explorer handles folders with 50,000+ files without stalling. If someone opens C:\Windows\System32 (~5,000 files), does the current app stay responsive? If not, is "Polish" the right label, or is this actually a Core correctness requirement for any credible Explorer replacement?
This is one of the most important point to improve from our current version.

Q13. Is the sidebar/breadcrumb experience complete enough for daily use?
Clickable breadcrumb segments are marked MVP and unchecked. Hidden-file toggle is MVP and unchecked. These are interactions Explorer users perform dozens of times per day. What percentage of a power user's typical Explorer session can Filer currently complete without hitting an unchecked MVP item?
It should be usable, but this version isnt final, we are wanting to rewrite or at least improve on many UX side of this project later, at least we need to finish fill in all the tech dept first.

---
V. Process & Sustainability

Q14. What is the maintenance cost of documentation that isn't synchronized with code?
The main README.md still references filer-gui/ (a directory that no longer exists — renamed to filer-app/). The BUGS.md is a single unstructured file with no dates, no owners, and no "fixed" labels. ROADMAP items accumulate but nothing is removed. Is there a process for keeping documentation honest, or does it drift toward aspirational marketing?
It currently drift toward any idea that spark by the writer.

Q15. What should be deleted or explicitly deferred to reduce the target surface?
If you had to cut 30% of the total roadmap items to guarantee the remaining 70% ship at high quality, what would you cut? The answer to this question probably reveals what the project actually is — a polished local Explorer replacement, a platform for developer tooling, or an extensible VFS framework.
We might cut on vfs, many smart feature like cache, pipeline might got a hit, the ecosystem might be delete first though.

---
Synthesis Question

Q16. What is the one thing that, if left unchecked for another six months, kills the project's momentum — scope creep, reliability debt, missing MVP basics, or no path to a user-distributable build?
This is my personal project so it should be the passionate it self or if microslop decide to improve file explorer to match my expectation.