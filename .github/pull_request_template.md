## Summary

<!-- A clear, concise description of what this PR does. What problem does it solve? -->



## Type of Change

<!-- Check all that apply -->

- [ ] 🐛 Bug fix (non-breaking change that fixes an issue)
- [ ] ✨ New feature (non-breaking change that adds functionality)
- [ ] 💥 Breaking change (fix or feature that changes existing behaviour)
- [ ] 📚 Documentation update
- [ ] 🧪 Tests (adding or improving test coverage)
- [ ] ♻️ Refactor (no functional changes)
- [ ] 🔧 Chore (dependency update, build tooling, CI)

## Related Issue

<!-- Link the issue this PR addresses. Use "Closes #NNN" to auto-close it on merge. -->

Closes #

## Changes Made

<!-- List the key changes in this PR. Be specific enough that a reviewer knows where to look. -->

- 
- 
- 

## Testing Done

<!-- Describe how you verified your changes work correctly. -->

- [ ] `cargo fmt` — code is formatted
- [ ] `cargo clippy -- -D warnings` — no Clippy warnings
- [ ] `cargo test` — all tests pass
- [ ] Manually tested on macOS (version: _________)
- [ ] Tested the daemon lifecycle (`clipwallet run` → hotkeys → `clipwallet stop`)
- [ ] Tested relevant hotkey chords end-to-end

<!-- Describe any additional manual testing steps you ran: -->



## Screenshots / Screen Recording

<!-- If your change affects observable behaviour (CLI output, paste results, mode switching, etc.), add a screenshot or recording here. Delete this section if not applicable. -->



## Breaking Changes

<!-- If this is a breaking change, describe exactly what breaks and what users need to do to migrate. Delete this section if not applicable. -->



## Checklist

- [ ] My branch is up to date with `main` (`git fetch upstream && git rebase upstream/main`)
- [ ] My commits follow the [Conventional Commits](https://www.conventionalcommits.org/) format
- [ ] I have updated relevant documentation (README, inline docs, CHANGELOG) if needed
- [ ] I have not logged clipboard contents anywhere, even at debug level
- [ ] This PR is focused — it addresses one issue or feature