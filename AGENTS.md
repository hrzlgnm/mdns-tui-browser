# AGENTS.md

This file contains guidelines and commands for agentic coding agents working in this repository.

## Build Commands

### Development
```bash
# Run the application in development mode
cargo run

# Run with custom service types
cargo run -- --service-types "_http._tcp.local.,_ssh._tcp.local."
```

### Building
```bash
# Build optimized release version
cargo build --release

# Build with audit trail (used in CI for releases)
cargo auditable build --release
```

### Testing
```bash
# Run all tests using nextest (preferred)
cargo nextest run --profile ci

# Run all tests using standard cargo test
cargo test

# Run a single test
cargo nextest run --profile ci test_name

# Run tests matching a pattern
cargo nextest run --profile ci test_pattern

# Run tests for a specific module
cargo nextest run --profile ci tui_app::tests
```

### Linting and Formatting
```bash
# Format code (will check CI)
cargo fmt

# Check formatting (fails in CI if not formatted)
cargo fmt -- --check

# Run clippy lints (fails in CI on warnings)
cargo clippy --tests -- -D warnings

# Check for typos
cargo install typos-cli
typos

# Check GitHub Actions workflows and reusable actions
actionlint

# Validate renovate configuration
docker run --rm --volume=$(pwd):$(pwd):ro --workdir=$(pwd) kokuwaio/renovate-config-validator:latest
```

## Code Style Guidelines

### Safety Policy
- **FORBIDDEN**: No `unsafe` blocks allowed anywhere in the codebase
- **REQUIRED**: `#![forbid(unsafe_code)]` at the top of every Rust file
- This is a **Safe Rust Only** project - memory safety is non-negotiable

### Imports and Dependencies
- Use `use` statements at the top of files in alphabetical order
- Group imports: std library, external crates, local modules
- Preferred libraries used in this project:
  - `ratatui` for TUI
  - `tokio` for async runtime
  - `crossterm` for terminal handling
  - `flume` for async channels
  - `mdns_sd` for service discovery
  - `clap` for CLI parsing
  - `chrono` for date/time handling

### Code Formatting
- Use `rustfmt` with default settings
- Maximum line length: 100 characters (rustfmt default)
- 4-space indentation (rustfmt default)
- Use `cargo fmt -- --check` to verify formatting

### Naming Conventions
- **Types**: `PascalCase` (structs, enums, type aliases)
- **Functions**: `snake_case`
- **Variables**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`
- **Enums**: PascalCase for enum name, PascalCase for variants
- **Fields**: `snake_case` for struct fields

### Error Handling
- Use `Result<T, Box<dyn std::error::Error>>` for main functions
- Prefer `Option<T>` for values that may be absent
- Use `?` operator for error propagation
- Avoid panic! except in unrecoverable situations
- Use `unwrap()` only in tests or when absolutely certain

### Async/Concurrency Patterns
- Use `tokio::sync::RwLock` for shared state
- Use `flume` channels for async communication
- Mark async functions with `async`
- Use `.await` for async operations
- Prefer `Arc<RwLock<T>>` for shared mutable state

### Testing Guidelines
- Write unit tests in `#[cfg(test)]` modules
- Use descriptive test names following `test_functionality_expected_result` pattern
- Use `assert_eq!`, `assert!`, `assert_ne!` for assertions
- Test edge cases and error conditions
- Integration tests go in `tests/` directory (if present)
- Use `cargo nextest` for faster test execution
- Test configuration in `.config/nextest.toml`

### TUI Specific Patterns
- Use `ratatui` for all UI components
- Handle events with `crossterm::event`
- Use `ListState` for selection state management
- Separate UI logic from business logic
- Use `Frame<'_>` for rendering
- Follow the existing app structure: `AppState` for state, `run_tui` for main loop

### Documentation
- Add doc comments to public functions with `///`
- Use `///` for module-level documentation
- Include examples in doc comments when helpful
- Keep documentation concise and focused
- Document all key bindings in README

### Performance Considerations
- Use `--release` builds for performance testing
- Profile with appropriate tools if needed
- Consider allocation patterns in hot paths
- Use `BTreeMap`/`BTreeSet` when ordering matters
- Use `HashMap`/`HashSet` for O(1) lookups when order doesn't matter

## Project Structure

```
src/
├── main.rs          # Entry point, CLI argument parsing
├── tui_app.rs       # Main TUI application logic and tests
├── input.rs         # User input handling (filter, service type)
├── popup.rs         # Popup UI components (help, metrics)
├── scroll.rs        # Scroll state management
├── models.rs        # Data models
└── terminal.rs     # Terminal handling
```

## Development Workflow

1. REQUIRED: Create a branch for your changes with an appropriate prefix (e.g., `feat/`, `fix/`, `chore/`, `refactor/`, `docs/`)
2. Make changes to source code
3. Run `cargo fmt` to format code
4. Run `cargo clippy --tests -- -D warnings` to check for issues
5. Run `cargo nextest run --profile ci` to run tests
6. Run `cargo build --release` to build release version
7. Run `cargo clippy --release -- -D warnings` to ensure no warnings in release
 8. Run `actionlint` to check GitHub Actions workflows if modified
 9. Run renovate config validator if `.github/renovate.json5` was modified
10. Test the application manually with `cargo run`
11. If README.md was updated, update the manpage (`docs/mdns-tui-browser.1`)
12. Commit logical units of work as you go, once all checks pass (see [Commits and Pull Requests](#commits-and-pull-requests))
13. Use conventional commit format (e.g., `feat:`, `fix:`, `docs:`) for commit messages
14. **REQUIRED**: After committing, immediately push the branch and create a pull request - do not wait for the user to ask

## Commits and Pull Requests

These conventions are shared across repositories. They describe the standing
authorization to commit as you go and the structure expected of every commit.

### Branches and PRs
- Conventional commits: `feat:`, `fix:`, `chore:`, `refactor:`, `docs:`, etc.
- Tags follow `vMAJOR.MINOR.PATCH`.
- All changes land via pull requests on a branch; direct pushes to `main` are
  blocked. Create a `feat/...`, `fix/...`, etc. branch and open a PR. After
  committing, push and open the PR without waiting to be asked.
- PRs are squash-merged, so the fine-grained structure below exists for review
  clarity, not for the final history.

### When to commit
- Do not leave completed work uncommitted. Once a logical unit of work is done
  and the tree is green, commit it — don't wait to be asked. This is a standing
  authorization: treat every task as implicitly including "and commit your
  work" unless the user says otherwise.
- Commit as you go, not all at once at the end. If a task naturally splits into
  an independent prep refactor plus a behavior change, that's two (or more)
  commits made in that order. Tests for a behavior change belong in the same
  commit as the change itself, not a separate one.

### How to structure commits
- Prefer a fine-grained history. Commits should be as small as possible while
  still meaningful and self-contained.
- Every commit must compile and pass all tests. No "WIP" commits, and no commits
  that leave the tree broken pending a follow-up.
- Every commit must be formatted and lint-clean: run `cargo fmt`,
  `cargo clippy --tests -- -D warnings`, and `cargo nextest run --profile ci`
  before committing. Don't introduce a warning in one commit and rely on a later
  commit to clean it up.
- Commit messages explain *why*, not *what*. The diff already shows what
  changed; the message should capture the motivation, the constraint, or the bug
  being fixed. If the reason is obvious from a one-line subject, no body is
  needed — but never paraphrase the diff.
- Separate preparatory refactors from behavior changes. If a fix or feature is
  easier to review after a refactor, land the refactor in its own commit first.
  The commit that changes behavior should be as small as possible. This applies
  even when the refactor only becomes apparent mid-change.
- Wrap the message body at 72 characters. The subject may run up to ~80
  characters.

### Attributing AI usage
- Every commit gets both trailers in a trailer block after a blank line. Use
  `--trailer` on the command line so no wrapping or manual formatting is needed:
  - `Co-authored-by: opencode <noreply@opencode.ai>`
  - `Assisted-by: opencode (<model-name>)`
- Trailers are exempt from the 72-character body wrap.
- Never use `--author` or `--committer` for attribution; release-notes tooling
  derives the credited user from the commit author, so doing so would replace
  the user with the bot throughout the release notes.

### Iterate with fixup!/amend! commits
- When refining already-committed work, create a fixup against the target
  (`git commit --fixup=<sha>`) so it sits alongside its target, ready for the
  user to fold in with `git rebase --autosquash`. Don't pile follow-up commits
  on top intending to squash them later.
- Even when the target is HEAD, use `git commit --fixup`, not
  `git commit --amend`. A bare `--amend` rewrites history on the spot and skips
  the reviewable checkpoint a fixup provides.
- If a change would make the target's message inaccurate, use
  `git commit --fixup=amend:<sha>` and revise the message in the prefilled
  editor. The replacement message must repeat the subject as its first line
  (`amend! <original subject>`) and then provide the new subject and body.
- Never squash fixups yourself. Leave them in history as separate commits for
  the user to review and fold in. Don't run `git rebase --autosquash` or
  `--amend` them into their targets. Because this repo squash-merges PRs, a
  target plus its fixups becomes one squashed commit at merge — the fixup
  discipline is purely for review visibility.

## Engineering Norms

### Surfacing decisions
When a decision surfaces while implementing — a design choice, a tradeoff, a
scope cut, or an unforeseen bug, race, or wrong assumption — stop and lay out
the options and your recommendation; let the user weigh in. Obvious mechanical
choices with one sensible answer don't need a checkpoint, but genuine forks do:
ones where a reasonable person might pick differently, or where you'd trade away
something the plan assumed (scope, UX, performance, …). This applies to
discoveries too — finding a latent bug is itself the fork: whether to fix it
here or in a separate change is the user's call to make with you.

### Don't present "live with the bug" as an option
When investigating a defect and laying out fix options, "accept the race / leave
it as-is / document it and move on" is not a valid option. A known race
condition, data corruption, or correctness violation needs a real fix. If a real
fix is genuinely out of reach, say so plainly rather than dressing "no fix" up
as a viable choice.

### Prefer the cleaner design over the smaller diff
When a task could be implemented either by tacking onto existing code or by
first restructuring it slightly, choose the restructuring. "Minimal change" is
not a goal in itself; a readable final state is. The prep-refactor-then-behavior
change pattern exists for exactly this. This is not license for speculative
abstraction — but if the current change would be clearer after extracting a
method, splitting a function, or adjusting names, that refactor is part of the
task, not an optional extra.

### Code comments
Comments explain *why* the code is shaped this way, not the path taken during
development (what was tried first, what's "cleaner" than the old approach). The
iteration story belongs in the commit message, not the code. Before writing a
comment, ask: would I have written this if writing the file from scratch with no
diff in mind? If not, it belongs in the commit message. Also avoid justifying
routine call sites: if neighboring call sites are bare, match them.

## Packaging

### AUR Packaging Tests
Run these commands from the repository root:

```bash
# Test source and binary packages
~/.local/bin/test-aur-local --variant=both

# Test one package variant
~/.local/bin/test-aur-local --variant=source
~/.local/bin/test-aur-local --variant=bin
```

- Use `--no-build` only for generator and lint smoke tests; omit it to test package creation and installation.
- Use `--no-install` to skip installing the `-bin` package, and `--no-cleanup` or `--keep-dir=<path>` to retain build artifacts for debugging.

### Changelog Inclusion

`CHANGELOG.md` is generated automatically by `git-cliff` from the conventional
commit messages (configured in `cliff.toml` and `.github/cliff-release.toml`).
**Never edit `CHANGELOG.md` by hand** — a correct conventional commit (see
[Commits and Pull Requests](#commits-and-pull-requests)) is the only input that
drives the changelog. Do not add manual "Unreleased" entries; the release
tooling derives them from commits at tag time.

When adding or modifying packaging configurations, ensure `CHANGELOG.md` is
included so the generated file ships with the package:
- **Debian packages**: Add `target/debian/changelog.gz` → `usr/share/doc/mdns-tui-browser/changelog.gz` as an asset in `Cargo.toml` `[package.metadata.deb]`. The build workflow gzip-compresses `CHANGELOG.md` before `cargo deb` runs. Do **not** use the `changelog` field — it expects Debian-format changelogs, not upstream markdown.
- **AUR packages**: Install `CHANGELOG.md` to `/usr/share/doc/$pkgname/` in the `package()` function
- **Release archives** (tar.gz/zip): Copy `CHANGELOG.md` into the staging directory in `build-reusable.yml`
- **macOS DMGs**: Copy `CHANGELOG.md` into the app bundle's `Contents/Resources/`

## Documentation Maintenance

### Manpage Updates
The manpage (`docs/mdns-tui-browser.1`) must be kept in sync with `README.md`. When updating documentation:

1. **CLI Options**: Update both README.md CLI Options section and manpage OPTIONS section
2. **Controls**: Update both README.md Controls section and manpage CONTROLS section  
3. **Service Types**: Update both README.md Service Types section and manpage SERVICE TYPES section
4. **Examples**: Update both README.md Examples section and manpage EXAMPLES section
5. **Date**: REQUIRED - Update the manpage date to current date in YYYY-MM-DD format in the .TH header

The manpage should contain only essential usage information without excessive detail, focusing on what users need to know to use the program effectively.

## CI/CD Integration

- Tests run on Ubuntu, macOS, and Windows in CI
- Nextest generates JUnit XML reports
- Formatting and clippy must pass before merging
- Release builds use `cargo auditable` for security
- Typos checked with `typos-cli` configuration in `typos.toml`
- GitHub Actions workflows validated with `actionlint`
- Renovate configuration validated with `renovate-config-validator`

## Common Pitfalls to Avoid

- **Never** use `unsafe` code - this will cause CI to fail
- **Never** persist GitHub credentials in Actions checkouts - set `persist-credentials: false` on `actions/checkout` unless the job pushes to the repo
- **Never** add `#[allow(warnings)]` attributes to suppress warnings - fix the underlying issues instead
- **Never** rewrite published history with `git commit --amend` - use `git commit --fixup` for iterations (see [Commits and Pull Requests](#commits-and-pull-requests)). Leave squashing to the user; don't run `git rebase --autosquash` or `--amend` fixups into their targets.
- **Always** format code before committing
- **Always** run clippy and fix warnings (both debug and release)
- **Don't** add dependencies without updating Cargo.toml properly
- **Don't** break the async patterns used throughout the codebase
- **Don't** ignore test failures - all tests must pass
- **Don't** have warnings in release builds - run `cargo clippy --release` before committing
- **Don't** add new CLI options without including them in the JSON state dump - see [State Dump Management](#state-dump-management)

## State Dump Management

When adding new CLI options, ensure they are included in the JSON state dump for full state restoration:

1. **Add to `AppOptions`** in `src/models.rs`:
   - Add the field to the `AppOptions` struct
   - Ensure it has proper `Serialize`/`Deserialize` derive

2. **Update `AppState`** in `src/tui_app.rs`:
   - Add the field to the `AppState` struct
   - Update the `Clone` impl
   - Pass through `AppState::new()` constructor

3. **Update state dump functions** in `src/tui_app.rs`:
   - Update `create_state_dump()` to include the new option
   - Update `load_from_state_dump()` to restore the new option

4. **Maintain backward compatibility**:
   - Use `#[serde(default)]` on fields in `AppOptions`
   - Test loading old state dumps without the new field

## Specific Notes for This Project

- mDNS service discovery is the core functionality
- TUI responsiveness is critical - avoid blocking operations
- Service filtering and sorting are key features
- Real-time updates should not block the UI thread
- Memory usage should be reasonable for long-running sessions
- Error messages should be user-friendly in the TUI context
