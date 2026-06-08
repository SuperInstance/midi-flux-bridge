# Contributing

Thank you for your interest in contributing! Here are the guidelines:

## Development Setup

```bash
git clone https://github.com/SuperInstance/midi-flux-bridge.git
cd midi-flux-bridge
cargo test
```

## Making Changes

1. Fork the repository.
2. Create a feature branch (`git checkout -b feature/my-feature`).
3. Make your changes with clear commit messages.
4. Ensure all tests pass: `cargo test`.
5. Ensure no clippy warnings: `cargo clippy -- -D warnings`.
6. Ensure formatting is correct: `cargo fmt --check`.
7. Add tests for any new functionality.
8. Submit a pull request.

## Code Style

- Run `cargo fmt` before committing.
- All public items must have doc comments.
- Keep the API minimal and well-documented.

## Reporting Issues

- Use GitHub Issues.
- Include a minimal reproduction case.
- Specify your Rust version (`rustc --version`).

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
