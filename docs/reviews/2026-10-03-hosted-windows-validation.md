# Hosted Windows validation, 2026-10-03

Windows filesystem work needs native execution to validate NTFS behavior and
measure directory listing costs. This review identifies hosted systems that can
provide that execution without a local Windows computer. It separates automated
core validation from work that needs a controlled desktop session.

## Choose the host

| Host | Fit for Filer | Constraint |
|---|---|---|
| GitHub Codespaces | Linux development and workflow authoring | Codespaces runs Linux containers; Windows is not a supported remote container OS. |
| Standard GitHub Actions Windows runner | Native Rust tests and NTFS listing comparisons | A job gets a fresh VM, so before/after comparisons must share one job. |
| Persistent Azure Windows VM | Repeated debugging and desktop automation | Requires provisioning and payment; Windows client images have license eligibility requirements. |

These capabilities come from [Codespaces documentation](https://docs.github.com/en/codespaces/about-codespaces/what-are-codespaces),
[GitHub-hosted runner documentation](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
and the [Azure Windows VM quickstart](https://learn.microsoft.com/en-us/azure/virtual-machines/windows/quick-create-portal).
Codespaces can submit Windows Actions jobs, but its own filesystem cannot provide
native Windows validation.

Standard x64 runner images include `windows-2022` and `windows-2025`. Standard
public-repository jobs are free; private-repository jobs use included minutes
before metered billing. Pin an OS label and record its actual image version,
because `windows-latest` follows GitHub's image selection. Windows 11 arm64
labels also exist, but measure a different architecture.
See the [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## Finish filesystem measurements

[VFS-002](../../.tasks/core/VFS-002-enumerate-local-directory-pages-in-one-blocking-task.md)
can use a standard Windows Actions job for its remaining NTFS measurement.
The repository already includes `windows-latest` in its manual
[CI matrix](../../.github/workflows/ci.yml). Tests alone do not produce the
required performance evidence.

The smallest next step is a separate manual benchmark job on `windows-2022` or
`windows-2025`. Build both revisions from the
[Linux comparison](../../crates/filer-core/benches/baselines/2026-10-01-vfs-002-linux-i7-11800h-btrfs.md)
and run them sequentially inside that one job VM with the same fixtures and
release settings. Use the existing
[benchmark fixture controls](../../crates/filer-core/benches/README.md) to select
the measured volume. Check its filesystem with
[Get-Volume](https://learn.microsoft.com/en-us/powershell/module/storage/get-volume?view=windowsserver2025-ps)
and require NTFS rather than assuming it from the runner label.

Record first-page timing, allocations, Rust version, revisions, runner image,
CPU, filesystem, fixture parameters, and cache policy. Alternate sample order
when practical and preserve the raw output. This is a recommendation for
comparability: separate jobs receive separate VMs, and a cloud VM measurement
describes that VM profile rather than a consumer desktop's storage performance.

Closing VFS-002 also requires a measured decision about native enumeration
versus the batched `std::fs` path. Before/after timing of the existing change
does not answer that criterion. A native candidate must be compared on the same
Windows profile before either adding its dependency or recording no gain.

[CORE-041](../../.tasks/core/CORE-041-add-rust-primitive-baselines-and-reproducible-reports.md)
can use the same host for its Windows NTFS primitive and core baseline profile
once its prerequisite work is ready. Hosted Windows execution does not replace
the adapters, validation, and report generation specified by that task.

## Reserve desktop access for desktop work

[CORE-033](../../.tasks/core/CORE-033-add-external-application-adapters-and-comparative-reports.md)
requires File Explorer and Filesmash observations through UI Automation. It is
deferred. For that work, recommend a persistent Windows 11 desktop VM with a
controlled interactive session and pinned application versions. Windows Server
NTFS results do not establish Windows 11 desktop application behavior.

Azure provides Windows Server VMs with [RDP access](https://learn.microsoft.com/en-us/azure/virtual-machines/windows/connect-rdp).
For a Windows 11 dev/test VM, check the account's eligibility under
[Windows client image licensing](https://learn.microsoft.com/en-us/azure/virtual-machines/windows/client-images)
before selecting the image. Microsoft's desktop testing guidance requires
interactive execution for visible desktop tests and explains why RDP session
locking can break automation. That guidance concerns Azure Pipelines; this
review does not establish whether a particular GitHub-hosted image supports
the required UI Automation session.
See [UI testing considerations](https://learn.microsoft.com/en-us/azure/devops/pipelines/test/ui-testing-considerations?view=azure-devops).

An optional GitHub-managed alternative is a larger runner with the
[Base Windows 11 Desktop partner image](https://docs.github.com/en/actions/reference/runners/larger-runners).
[Larger runners](https://docs.github.com/en/actions/concepts/runners/larger-runners)
require an organization on GitHub Team or Enterprise Cloud and are always
metered, including public repositories. Its image alone does not prove the
desktop session meets CORE-033's observation requirements.
