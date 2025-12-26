# Contributing to caxe

Thank you for your interest in contributing to caxe! 🎉

## Getting Started

1. **Fork and clone** the repository
   ```bash
   git clone https://github.com/YOUR_USERNAME/caxe.git
   cd caxe
   ```

2. **Build the project**
   ```bash
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

4. **Check code quality**
   ```bash
   cargo clippy  # Should have 0 warnings
   cargo fmt --check
   ```

## Development Workflow

### Adding a New Feature

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Write tests for your feature in the appropriate test module
3. Implement your feature
4. Run `cargo test && cargo clippy` to verify
5. Submit a pull request

### Fixing a Bug

1. Create a bug branch: `git checkout -b fix/issue-123`
2. Add a test that reproduces the bug
3. Fix the bug
4. Verify the test passes
5. Submit a pull request

## Code Style

- Run `cargo fmt` before committing
- All public functions should have doc comments (`///`)
- Use `anyhow::Result` for error handling
- Follow the existing code structure

## Testing

We have three types of tests:

| Type | Location | Command |
|------|----------|---------|
| Unit | `src/*.rs` | `cargo test --lib` |
| Integration | `tests/` | `cargo test --test integration_test` |
| Benchmarks | `benches/` | `cargo bench` |

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        assert_eq!(my_function(), expected_value);
    }
}
```

## Pull Request Guidelines

- Keep PRs focused on a single feature/fix
- Include tests for new functionality
- Update documentation if needed
- Ensure CI passes

## Reporting Issues

When reporting bugs, please include:

- caxe version (`cx --version`)
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Relevant `cx.toml` configuration (if applicable)

## License

By contributing, you agree that your contributions will be licensed under the MIT OR Apache-2.0 license.

---

**Thank you for helping make caxe better!** 🪓
