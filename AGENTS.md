# Weaver Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-01-01

## Active Technologies

- Rust 1.92+ + `clap` (CLI), `serde` (YAML/JSON), `tera` (Templating), `copy_dir` (File ops), `dialoguer` (Prompts) (001-repo-weaver-mvp)
- **Logging**: `tracing` (Instrument/Facades) + `tracing-subscriber` (Fmt/EnvFilter) for structured logging.

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust 1.92+: Follow standard conventions

## Recent Changes

- 001-repo-weaver-mvp: Added Rust 1.92+ + `clap` (CLI), `serde` (YAML/JSON), `tera` (Templating), `copy_dir` (File ops), `dialoguer` (Prompts)

<!-- MANUAL ADDITIONS START -->
- Test-driven development (TDD) for all features (from unit to e2e, full spectrum. try to avoid mocks). 1. Understand, what type of test is required for a step. 2. Write a minimal failing test. 3. Write a minimal implementation that makes the test pass. 4. Refactor the implementation to make it better. 5. Repeat.
- Think before implementing anything, if it's not already implemented.
- Think carefully about the code structure and organization before implementing anything.
<!-- MANUAL ADDITIONS END -->
