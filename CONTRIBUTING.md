# Contributing to Monarch-DB

Thank you for your interest in contributing to Monarch-DB! This document provides guidelines and information for contributors.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Python 3.7+ (for pre-commit hooks)
- Git

### Setting up the Development Environment

1. **Clone the repository:**

   ```bash
   git clone https://github.com/alexrudy/monarch-db.git
   cd monarch-db
   ```

2. **Install Rust dependencies:**

   ```bash
   cargo build
   ```

3. **Set up pre-commit hooks:**

   ```bash
   ./scripts/setup-pre-commit.sh
   ```

   Or manually:

   ```bash
   pip install pre-commit
   pre-commit install
   ```

4. **Run tests to ensure everything works:**

   ```bash
   cargo test
   ```

## Code Style and Quality

This project uses several tools to maintain code quality:

### Pre-commit Hooks

The project uses pre-commit hooks to automatically check and fix common issues:

- **Rust formatting** (`cargo fmt`)
- **Rust linting** (`cargo clippy`)
- **Documentation generation** (`cargo doc`)
- **Testing** (`cargo test`)
- **Markdown linting**
- **YAML validation**
- **Spell checking**
- **Secret detection**

### Running Quality Checks

```bash
# Run all pre-commit hooks
pre-commit run --all-files

# Run specific checks
cargo fmt --check      # Check formatting
cargo clippy           # Run linter
cargo test             # Run tests
cargo doc              # Build documentation

# Run manual checks
pre-commit run --hook-stage manual cargo-audit  # Security audit
pre-commit run --hook-stage manual cargo-deny   # Dependency analysis
```

### Code Formatting

- Use `cargo fmt` to format Rust code
- Follow the default rustfmt configuration
- Lines should generally be under 100 characters

### Documentation

- Add rustdoc comments for all public items
- Include examples in documentation where helpful
- Keep the README up to date with new features

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with features
cargo test --features serde

# Run specific test suites
cargo test --lib                           # Unit tests
cargo test --test static_configuration     # Static config integration tests
cargo test --test directory_configuration  # Directory config integration tests

# Test the CLI
cargo build --release
./target/release/monarch help
```

### Writing Tests

- Add unit tests for new functionality
- Include integration tests for significant features
- Test both happy path and error conditions
- Use descriptive test names

## Submitting Changes

### Pull Request Process

1. **Fork the repository** on GitHub
2. **Create a feature branch** from `main`:

   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **Make your changes** following the guidelines above
4. **Add tests** for new functionality
5. **Update documentation** if needed
6. **Run the test suite**:

   ```bash
   cargo test
   pre-commit run --all-files
   ```

7. **Commit your changes** with a clear message
8. **Push to your fork** and **create a pull request**

### Commit Messages

Use clear, descriptive commit messages:

```text
Add CLI support for migration status checking

- Implement `monarch version` command
- Add integration tests for CLI operations
- Update README with CLI documentation
```

### What to Include in Pull Requests

- **Clear description** of what the change does
- **Rationale** for why the change is needed
- **Testing notes** - how you tested the change
- **Breaking changes** - if any, clearly documented
- **Documentation updates** - if needed

## Issue Reporting

### Bug Reports

When reporting bugs, please include:

- **Rust version** (`rustc --version`)
- **Operating system** and version
- **Steps to reproduce** the issue
- **Expected behavior** vs **actual behavior**
- **Error messages** or logs if applicable
- **Sample code** that demonstrates the issue

### Feature Requests

For feature requests, please include:

- **Use case** - what problem does this solve?
- **Proposed solution** - if you have ideas
- **Alternatives** - other ways to solve the problem
- **Examples** - how would the feature be used?

## Code of Conduct

This project follows a simple code of conduct:

- **Be respectful** and considerate in all interactions
- **Be collaborative** and help others learn
- **Be patient** with questions and different skill levels
- **Be constructive** in feedback and criticism

## Getting Help

If you need help or have questions:

- **Check existing issues** to see if your question has been answered
- **Create a new issue** with the `question` label
- **Join discussions** in existing issues and pull requests

## Release Process

Releases are handled by maintainers:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` (if maintained)
3. Create a git tag
4. Publish to crates.io
5. Create GitHub release

## License

By contributing to Monarch-DB, you agree that your contributions will be licensed under the same license as the project (MIT).
