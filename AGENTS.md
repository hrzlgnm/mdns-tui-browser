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
└── ...              # Additional modules if added
```

## Development Workflow

1. REQUIRED: Create a branch for your changes 
2. Make changes to source code
3. Run `cargo fmt` to format code
4. Run `cargo clippy --tests -- -D warnings` to check for issues
5. Run `cargo nextest run --profile ci` to run tests
6. Test the application manually with `cargo run`
7. If README.md was updated, update the manpage (`mdns-tui-browser.1`)
8. Commit only when all checks pass
9. After committing, push to the repository and create a pull request if applicable
10. After the pull request is created, add a comment `@coderabbitai review`
11. After pushing changes to an existing pull request, add a comment `@coderabbitai review` to trigger a new review

## Documentation Maintenance

### Manpage Updates
The manpage (`mdns-tui-browser.1`) must be kept in sync with `README.md`. When updating documentation:

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

## Common Pitfalls to Avoid

- **Never** use `unsafe` code - this will cause CI to fail
- **Always** format code before committing
- **Always** run clippy and fix warnings
- **Don't** add dependencies without updating Cargo.toml properly
- **Don't** break the async patterns used throughout the codebase
- **Don't** ignore test failures - all tests must pass

## Specific Notes for This Project

- mDNS service discovery is the core functionality
- TUI responsiveness is critical - avoid blocking operations
- Service filtering and sorting are key features
- Real-time updates should not block the UI thread
- Memory usage should be reasonable for long-running sessions
- Error messages should be user-friendly in the TUI context
