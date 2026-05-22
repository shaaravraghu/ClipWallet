# Contributing to ClipWallet

Thank you for your interest in contributing to ClipWallet! This project is part of **GirlScript Summer of Code (GSSoC) 2026**, and we welcome contributions of all kinds — bug fixes, features, documentation, and testing.

---

## Table of Contents

- [Getting Started](#getting-started)
- [Local Setup](#local-setup)
- [Branch Naming Conventions](#branch-naming-conventions)
- [Commit Message Format](#commit-message-format)
- [Pull Request Workflow](#pull-request-workflow)
- [Coding Standards](#coding-standards)
- [Finding Issues to Work On](#finding-issues-to-work-on)

---

## Getting Started

1. **Fork** the repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/<your-username>/ClipWallet.git
   cd ClipWallet
   ```
3. **Add the upstream remote** so you can stay in sync:
   ```bash
   git remote add upstream https://github.com/shaaravraghu/ClipWallet.git
   ```

---

## Local Setup

### Prerequisites

ClipWallet is a **macOS-only** Rust project. Before building, ensure you have:

| Requirement | Version | Install |
|-------------|---------|---------|
| macOS | 12 Monterey+ | — |
| Rust toolchain | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Xcode Command Line Tools | latest | `xcode-select --install` |

You can verify your Rust installation with:
```bash
rustc --version
cargo --version
```

### Build & Run

```bash
# Build in development mode
cargo build

# Build an optimised release binary
cargo build --release

# Run the daemon directly
cargo run -- run

# Run with debug logging enabled
RUST_LOG=debug cargo run -- run
```

### Code Quality Checks

Always run these before opening a PR:

```bash
# Format code
cargo fmt

# Run the linter (warnings are treated as issues)
cargo clippy -- -D warnings
```

### Running Tests

```bash
cargo test
```

> **Note:** Some integration tests require macOS Accessibility permissions to be granted to the terminal. See `MANUAL_INSTALLATION.md` for permission setup steps.

---

## Branch Naming Conventions

Create your working branch off `main` using one of these prefixes:

| Type | Pattern | Example |
|------|---------|---------|
| New feature | `feature/<short-description>` | `feature/custom-hotkey-config` |
| Bug fix | `fix/<short-description>` | `fix/vault-key-rotation-crash` |
| Documentation | `docs/<short-description>` | `docs/improve-readme-onboarding` |
| Tests | `test/<short-description>` | `test/static-store-unit-tests` |
| Refactor | `refactor/<short-description>` | `refactor/engine-error-handling` |
| Chore/tooling | `chore/<short-description>` | `chore/update-cargo-deps` |

```bash
git checkout -b feature/clipboard-search
```

---

## Commit Message Format

ClipWallet follows the [Conventional Commits](https://www.conventionalcommits.org/) specification. This keeps the Git history readable and powers automated changelogs.

```
<type>(<optional scope>): <short summary>

<optional body — explain the why, not the what>

<optional footer — issue refs, breaking change notices>
```

### Types

| Type | When to use |
|------|------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation-only changes |
| `style` | Formatting, missing semicolons — no logic change |
| `refactor` | Code change that is neither a fix nor a feature |
| `test` | Adding or updating tests |
| `chore` | Build process, dependency updates, tooling |
| `perf` | Performance improvements |

### Examples

```bash
# Good
git commit -m "feat(engine): add clipboard search across all slots"
git commit -m "fix(vault): handle missing Keychain entry on first launch"
git commit -m "docs: add architecture diagram to README"
git commit -m "chore: bump aes-gcm to 0.10.3"

# Bad — too vague
git commit -m "fix stuff"
git commit -m "updates"
```

Breaking changes must be noted in the footer:
```
feat(hotkey)!: change default chord prefix from Cmd+Opt to Cmd+Shift

BREAKING CHANGE: existing user hotkey muscle memory will need updating.
```

---

## Pull Request Workflow

1. **Sync your fork** before starting work:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Make your changes** on your feature branch. Keep PRs **focused** — one feature or fix per PR makes review much faster.

3. **Run checks locally** before pushing:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```

4. **Push** your branch and open a PR against `main`:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Fill in the PR template** completely. PRs with empty sections will be asked for more detail before review begins.

6. **Link the issue** your PR addresses using `Closes #<issue-number>` or `Fixes #<issue-number>` in the PR description.

7. **Address review feedback** by pushing new commits to the same branch — do not force-push after a review has started unless asked by a maintainer.

8. A PR requires **at least one maintainer approval** before it can be merged.

---

## Coding Standards

ClipWallet is written in idiomatic Rust. Please follow these guidelines:

- **`cargo fmt`** — all code must be formatted. The CI will reject unformatted code.
- **`cargo clippy`** — all Clippy lints must pass. Add `#[allow(...)]` with a comment only as a last resort.
- **Error handling** — use `anyhow::Result` for application-level errors. Avoid `.unwrap()` in production paths; use `?` or meaningful error messages.
- **Async** — use `tokio` async patterns consistently. Do not block the async executor with synchronous calls.
- **Comments** — write doc comments (`///`) for all public functions and types. Add inline `//` comments for non-obvious logic, explaining the *why*, not the *what*.
- **Module boundaries** — keep modules focused. The layered architecture (hotkey → engine → storage) should be respected; avoid direct cross-layer coupling.
- **Tests** — add unit tests for any new pure logic. Integration tests for anything touching the OS clipboard or Keychain should be gated with `#[cfg(target_os = "macos")]`.
- **Security** — never log clipboard contents, even at `TRACE` level. Vault-related code changes require extra scrutiny.

---

## Finding Issues to Work On

- Browse [open issues](https://github.com/shaaravraghu/ClipWallet/issues) and look for the **`good first issue`** or **`help wanted`** labels.
- GSSoC participants: issues labelled **`GSSoC`** are pre-approved for the program.
- Before starting work on an issue, **leave a comment** to claim it and avoid duplicate effort.
- If you have an idea not covered by an existing issue, open a Feature Request first so we can discuss the approach before you invest time in an implementation.

---

## Need Help?

Join the [ClipWallet Discord](https://discord.gg/X8Hr8P9J) — the `#contributors` channel is the best place to ask questions, get unblocked, and discuss implementation ideas with maintainers.