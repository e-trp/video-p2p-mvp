# Working Guidelines for video-p2p-mvp

These instructions apply to the entire repository. More specific instructions in nested `AGENTS.md` files refine the rules for their directories. Explicit user instructions take priority.

## General Rules

- Before starting, read [README](README.md), [TODO](todo.md), the [bug register](docs/BUGS.md), [architecture](docs/ARCHITECTURE.md), [QA checklist](docs/QA_CHECKLIST.md), and the guides in `docs/` relevant to the change. Check `git status` and the current diff; preserve existing user changes.
- Prioritize current bugs over new features. Work on the highest-priority actionable bug first. A feature may take precedence only when its concrete implementation replaces or substantially redesigns the affected module/path so that an isolated fix would be discarded or the faulty behavior would change.
- Record that exception in the bug register before feature work: replacement feature ID, affected module/path, why an immediate fix would be superseded, target milestone, and the regression/acceptance check for the replacement. Keep the bug unresolved until verification establishes that the replacement removes it; unrelated enhancements and vague future refactoring do not justify deferral.
- Complete one coherent bug or TODO task at a time: implementation, test evidence, and current documentation. Keep unrelated refactoring separate. Maintain stable bug IDs linked to feature IDs, and update the register whenever a bug is discovered, investigated, deferred, fixed, or verified.
- Verify every behavior change with tests: add a regression scenario for a fix, and cover the normal path, errors, and meaningful boundaries for a feature. Updating an existing test is acceptable when it verifies the new behavior. Assert outcomes rather than duplicating the implementation.
- Document every completed change in the relevant guide and TODO. For documentation-only changes, verify links, anchors, commands, and consistency with the code; application tests created solely for Markdown are unnecessary. The automated checks below still apply.
- Track implementation, automated verification, and manual QA separately. Keep unverified scenarios open; scaffolding, mock/demo behavior, and RTP counters do not establish actual playback.
- Preserve component boundaries: capture contracts in `capture-core`, platform capture in `capture-macos`/`capture-linux`, codecs in `media-codec`, Rust WebRTC in `transport-webrtc`, orchestration in `app-core`, and UI/IPC in `apps/desktop`.
- Keep the media worker independent of GUI polling. The desktop viewer decodes media in the WebView; raw/encoded media bytes must not cross Tauri IPC. The Rust bridge handles signaling and diagnostics.
- Add dependencies and public APIs only for the current task. Explain their purpose, update the corresponding lockfile, and check platform constraints. Preserve each crate's edition and existing conventions.

## Project Language

- Write all project changes in English: documentation, TODO entries, code comments, rustdoc, authored UI/error text, test descriptions, commit messages, tag annotations, and release/QA records.
- When editing a file, translate any remaining Russian documentation or comments in that file. Before completing a change, scan tracked text files, including hidden configuration files, for Cyrillic text and resolve authored Russian prose. Preserve exact external data when a fixture requires it.
- Conversation with the user may follow the user's language; repository content remains English.

## Rust Style Guide

- Use standard `rustfmt`; avoid manual formatting that conflicts with it. Name functions/modules with `snake_case`, types/traits with `UpperCamelCase`, and constants with `SCREAMING_SNAKE_CASE`.
- Give functions and modules clear responsibilities. Keep visibility narrow: private by default, `pub(crate)` for internal connections, and `pub` for necessary APIs.
- Represent states with enums and distinct types when this prevents invalid combinations. Use `Option` for absence and `Result` for errors; avoid implicit sentinel values.
- Return contextual `Result` errors for expected OS/network/codec/input failures; propagate with `?`. Reserve `unwrap`/`expect` for tests and justified internal invariants, excluding external input and routine failures.
- Prefer borrowing, slices, and standard traits (`From`, `TryFrom`, `AsRef`, `Iterator`). Avoid unnecessary cloning, allocation, `Arc<Mutex<_>>`, and generics.
- Hold locks briefly; release them before `.await`, blocking I/O, encoding, or callbacks. Keep blocking work off the async executor through an appropriate worker or `spawn_blocking`.
- Use checked arithmetic and conversions for sizes, offsets, and timestamps. Validate incoming buffers and values; make time/bitrate units explicit in types or names.
- Document public APIs with rustdoc: behavior, constraints, and errors. Comments explain invariants and reasons for decisions.
- Resolve lint findings instead of hiding them with broad `#[allow(...)]` attributes. Explain any narrowly scoped exception.

References: [Rust Style Guide](https://doc.rust-lang.org/style-guide/) and [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html), applied within this project's architecture.

## Safe Rust and FFI

- Prefer safe Rust. Introduce `unsafe` only for a demonstrated FFI/low-level requirement without a suitable safe alternative. Borrow-checker convenience and unmeasured optimization are insufficient reasons.
- Isolate `unsafe` in minimal blocks inside platform/codec bridges with a verifiable safe API. Every block and `unsafe impl` needs a specific `// SAFETY:` explanation covering pointer validity, ownership/lifetimes, bounds/alignment, and synchronization. Document `unsafe fn` contracts under `# Safety` and use explicit internal unsafe blocks.
- At FFI boundaries, validate null pointers, lengths, overflow, and buffer formats before creating Rust references/slices. Use RAII for native handles; stop/drop must release resources and prevent callbacks after release.
- Prevent panic unwinding across C/Objective-C ABI boundaries. New `unsafe impl Send/Sync` declarations require a documented threading and synchronization model; an opaque handle alone is insufficient evidence.
- Verify FFI changes with extraction, validation, and lifecycle tests, including startup failure, stop/drop, and callback lifetimes. Look for opportunities to reduce existing unsafe sections.

See [Unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html). An external API or a comment alone does not establish safety.

## Implementation Order

[Current bugs](docs/BUGS.md#current-issues) take priority under the general rules above. [TODO](todo.md#not-done) defines ordered milestones, feature IDs, dependencies, and acceptance gates for feature work. After actionable bugs, follow its baseline QA prerequisites, then M1 through M7. Complete a vertical slice from input/capture through transport/UI and verification before opening the next dependent milestone.

If the user requests a specific task, perform it and update its status. Otherwise, choose the first available incomplete task. Preserve completed work. If a step needs an unavailable OS, permission, device, or network, record the limitation, leave its QA open, and continue the next independent available task. Synthetic probes cannot replace manual QA.

## Bug Tracking and Closure

- Use [docs/BUGS.md](docs/BUGS.md) as the source of truth for defects; TODO links to it rather than maintaining a second status list. Assign permanent `BUG-NNN` IDs and link each entry to a stable TODO feature ID. Use `BASE-*` IDs for existing baseline behavior/tooling; these IDs do not advance the new-feature counter.
- Record priority, status, issue kind, affected version/component, evidence, reproduction steps and reliability, expected/actual behavior, cause or explicit uncertainty, and closure criteria. Keep product defects, test failures, environment problems, missing features, and unperformed QA distinct.
- A blocked bug remains unresolved with its exact blocker and next action recorded. Work on the next actionable bug; blocked execution alone does not justify the module-replacement exception. Revisit deferred bugs whenever their replacement feature changes scope or reaches acceptance.
- For code defects, `FIXED` means an implementation exists with a fixing commit and regression evidence; `CLOSED` additionally requires the stated verification. For environment issues, record repair commands and verification rather than inventing a code fix/version. A passing retry, planned rewrite, workaround, or unreproduced failure alone does not close a bug. Preserve closed entries with their fixing version/commit where applicable and verification evidence.
- Bugs affecting required release scenarios block the version gate while unresolved, including deferred bugs. After replacement work, rerun the original scenario or a documented equivalent for the new implementation before closing the entry.

## Commits, Versions, and Git Tags

- The current project baseline is **`0.1.0`**. Keep the major version at **`0`** while the planned MVP is incomplete. Advance to `1.0.0` only after the required TODO scope, release/distribution work, and full QA pass; later versions follow SemVer compatibility rules.
- Use **annotated Git tags** named `vMAJOR.MINOR.PATCH`, such as `v0.2.0`. A tag identifies a verified release commit. Commit every completed coherent task after its checks; never accumulate five features into one large commit.
- Increment the minor number for a completed feature milestone and reset patch to zero (`0.1.0` → `0.2.0`). During `0.x`, record breaking changes and compatibility/migration implications in the minor release. Increment patch for a fix-only release (`0.2.0` → `0.2.1`); fixes can ship before another feature milestone. Documentation-only changes and behavior-preserving refactors require ordinary commits and no version bump.
- Prefer the independently testable functional blocks in TODO over arbitrary batches. Each block needs a clear outcome and acceptance gate; split oversized blocks before implementation. Run full-project QA at the end of each block, before every version tag, and **no later than every five newly completed features**. A milestone with fewer than five features still triggers full QA.
- A feature counts once by its stable TODO ID when its behavior, acceptance tests, documentation, and required feature-specific verification are complete. Subtasks, commits, tests, documentation, refactors, and bug fixes do not increase the counter. The existing completed inventory belongs to the `0.1.0` baseline and is not counted again.
- Maintain the current version, latest tag, completed feature IDs, and QA checkpoint state in [TODO's version checkpoint](todo.md#version-checkpoint). At five completed features, commit the last task and run the full checkpoint before extending the batch. Reset the counter only after a passing full QA checkpoint; record which IDs it covered. A completed block receives its version tag after the release gate passes.
- Full-project QA means reviewing **every section** of [QA_CHECKLIST](docs/QA_CHECKLIST.md), running all automated checks and supported manual scenarios, and recording results. Required `FAIL`, `BLOCKED`, or `NOT RUN` checks block tagging. `N/A` needs a scope-based reason; lack of a device, OS, permission, or network means `BLOCKED`, not `N/A`.
- Before tagging, synchronize all first-party package versions in `crates/*/Cargo.toml`, `apps/cli/Cargo.toml`, `apps/desktop/src-tauri/Cargo.toml`, both Cargo lockfiles, and `version`/`bundle.macOS.bundleVersion` in `apps/desktop/src-tauri/tauri.conf.json`. The root `Cargo.toml` is a workspace without a package version. Revalidate the final versioned tree, commit it with English release notes/QA evidence, then tag that exact commit with an English annotation listing the milestone, feature IDs, and QA record.
- Existing tags are immutable. Push commits/tags or publish releases only when requested. The untagged `0.1.0` baseline is not evidence of release QA; this documentation change does not create a release tag.

References: [Semantic Versioning](https://semver.org/spec/v2.0.0.html) and [Git annotated tags](https://git-scm.com/docs/git-tag).

## Documentation Maintenance

- After each completed change, review the affected guides, README, TODO, and relevant bug entries. Correct or remove outdated statements, commands, and duplication. If the existing content remains accurate, record that review in the final report.
- Keep README concise with status and entry points, `docs/` focused on each topic, and TODO focused on progress and remaining work. Avoid duplicating development history across them.
- Use `[x]` in TODO only under **FULLY COMPLETED**. Describe partial work as incomplete; use `[ ]` for missing implementation and outstanding QA, keeping them distinct.
- Update the appropriate implementation guide or validation/checklist. Record commands actually executed, results, and limitations; planned/skipped checks cannot be recorded as successful.
- Verify command examples, UI labels, defaults, and constraints against the code. Check relative links/anchors and shell-block syntax.

## Mandatory Checks for Every Completed Change

Run these commands from the repository root after the final edits. `apps/desktop/src-tauri` is a separate workspace; root commands do not cover it.

Format both workspaces and run strict lint:

```bash
cargo fmt --all
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo fmt --all -- --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Run all Rust tests in both workspaces, including doctests, and JS lifecycle tests:

```bash
cargo test --all --all-features
cargo test -p app-core --example playback_probe --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features
node --test apps/desktop/tests/viewer-player.test.mjs
```

On macOS, also run permission-free native extraction and all available browser playback/recovery probes:

```bash
sh scripts/test-macos-capture.sh
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-viewer-playback.mjs
node scripts/test-viewer-playback.mjs --keyframe-recovery
node scripts/test-viewer-playback.mjs --chrome
node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery
```

Browser probes require a macOS desktop session, Node 22+, local sockets, and codec/WebKit services; Chrome mode requires installed Chrome. On Linux, skip macOS-specific commands with an explicit platform reason. If the environment prevents a check, record the exact command/reason and leave it open; respect permission/OS constraints. Perform manual QA at milestone/five-feature/version checkpoints according to the [checklist](docs/QA_CHECKLIST.md#full-project-qa-and-version-gate), and whenever needed to verify a particular change.

Fix failures and rerun affected checks. Before completing work, run `git diff --check`, review the entire final diff, and exclude unrelated files from the commit. Report the result, test evidence, documentation updates, and remaining limitations.

## Local Artifact Cleanup

After tests, probes, and release-bundle work, save results and stop processes started for verification. Then clean **both** workspaces:

```bash
cargo clean
cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Cleanup is mandatory at the end of every completed change. Run it after all compile/test/probe steps and never alongside builds. If Cargo's target directory is overridden, check the cleanup scope and preserve shared caches used by other projects. Save any release bundle needed by the user outside the target directory before cleaning. Preserve registry/download caches, sources, lockfiles, and user preferences. Further tests/probes require rebuilding after clean. If cleanup fails due to ownership or permissions, report the exact failure and leave cleanup outstanding.

See [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html) for the artifacts removed.
