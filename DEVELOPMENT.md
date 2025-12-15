# Development Guide

This guide covers development setup, testing, and contribution guidelines for the Kraken WebSocket SDK.

## Table of Contents

- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Testing Strategy](#testing-strategy)
- [Performance Benchmarking](#performance-benchmarking)
- [Code Quality](#code-quality)
- [Contributing](#contributing)
- [Release Process](#release-process)

## Development Setup

### Prerequisites

- Rust 1.70+ (stable, beta, or nightly)
- Cargo (comes with Rust)
- Git

### Installation

1. Clone the repository:
```bash
git clone https://github.com/your-org/kraken-ws-sdk.git
cd kraken-ws-sdk
```

2. Install dependencies:
```bash
cargo build
```

3. Run tests to verify setup:
```bash
cargo test
```

### Development Tools

Install recommended development tools:

```bash
# Code formatting
rustup component add rustfmt

# Linting
rustup component add clippy

# Security auditing
cargo install cargo-audit

# Code coverage
cargo install cargo-llvm-cov

# Benchmarking (included in dev-dependencies)
# Documentation generation (built into cargo)
```

## Project Structure

```
kraken-ws-sdk/
├── src/                    # Source code
│   ├── lib.rs             # Library root
│   ├── client.rs          # Main WebSocket client
│   ├── connection.rs      # Connection management
│   ├── data.rs           # Data structures
│   ├── error.rs          # Error types
│   ├── events.rs         # Event system
│   ├── orderbook.rs      # Order book management
│   ├── parser.rs         # Message parsing
│   └── subscription.rs   # Subscription management
├── examples/              # Usage examples
│   ├── basic_usage.rs    # Basic SDK usage
│   ├── advanced_usage.rs # Advanced features
│   └── kraken_live_test.rs # Live connection test
├── tests/                # Test suites
│   ├── integration_tests.rs # Integration tests
│   ├── unit_tests.rs     # Unit tests
│   ├── property_tests.rs # Property-based tests
│   └── parser_tests.rs   # Parser-specific tests
├── benches/              # Performance benchmarks
│   └── performance_benchmarks.rs
├── scripts/              # Development scripts
│   └── run_tests.sh     # Comprehensive test runner
└── .github/workflows/    # CI/CD configuration
    └── ci.yml
```

## Testing Strategy

The SDK uses a comprehensive testing approach with multiple test types:

### 1. Unit Tests

Test individual components in isolation:

```bash
cargo test --test unit_tests
```

Coverage includes:
- Event dispatcher functionality
- Order book operations
- Subscription management
- Data structure validation
- Error handling

### 2. Integration Tests

Test component interactions:

```bash
cargo test --test integration_tests
```

Coverage includes:
- Client configuration and creation
- Callback registration and management
- Subscription workflow
- Configuration validation

### 3. Property-Based Tests

Test invariants and edge cases using QuickCheck:

```bash
cargo test --test property_tests
```

Coverage includes:
- Order book mathematical properties
- Data structure invariants
- Configuration validation
- Error handling consistency

### 4. Parser Tests

Test message parsing with real Kraken formats:

```bash
cargo test --test parser_tests
```

Coverage includes:
- Kraken message format parsing
- Malformed data handling
- Concurrent message processing
- Error recovery

### Running All Tests

Use the comprehensive test runner:

```bash
./scripts/run_tests.sh
```

Or run specific test categories:

```bash
# All tests
cargo test

# Specific test file
cargo test --test unit_tests

# Specific test function
cargo test test_event_dispatcher_creation

# With output
cargo test -- --nocapture
```

## Performance Benchmarking

The SDK includes comprehensive benchmarks using Criterion:

### Running Benchmarks

```bash
cargo bench
```

### Benchmark Categories

1. **Message Parsing**: Parser performance with different message types
2. **Order Book Operations**: Update and calculation performance
3. **Event Dispatching**: Callback invocation overhead
4. **Concurrent Operations**: Multi-threaded performance
5. **Message Size Impact**: Performance vs. message size

### Viewing Results

Benchmark results are saved to `target/criterion/` with HTML reports.

### Adding New Benchmarks

Add benchmarks to `benches/performance_benchmarks.rs`:

```rust
fn bench_new_feature(c: &mut Criterion) {
    c.bench_function("new_feature", |b| {
        b.iter(|| {
            // Benchmark code here
        })
    });
}
```

## Code Quality

### Formatting

Use rustfmt for consistent code formatting:

```bash
cargo fmt
```

### Linting

Use clippy for additional linting:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Documentation

Generate and view documentation:

```bash
cargo doc --open
```

### Security Auditing

Run security audits:

```bash
cargo audit
```

### Code Coverage

Generate coverage reports:

```bash
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

## Contributing

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/new-feature`
3. Make changes and add tests
4. Run the test suite: `./scripts/run_tests.sh`
5. Commit changes: `git commit -m "Add new feature"`
6. Push to your fork: `git push origin feature/new-feature`
7. Create a pull request

### Code Standards

- Follow Rust naming conventions
- Add documentation for public APIs
- Include tests for new functionality
- Maintain backward compatibility
- Update examples if needed

### Pull Request Guidelines

- Provide clear description of changes
- Include test coverage for new code
- Ensure CI passes
- Update documentation if needed
- Add changelog entry for significant changes

## Release Process

### Version Management

The SDK follows semantic versioning (SemVer):

- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes (backward compatible)

### Release Steps

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run full test suite: `./scripts/run_tests.sh --bench`
4. Create release commit: `git commit -m "Release v1.2.3"`
5. Tag release: `git tag v1.2.3`
6. Push changes: `git push origin main --tags`
7. Publish to crates.io: `cargo publish`

### Automated Releases

The CI pipeline automatically publishes releases when:
- Commit is on main branch
- Commit message contains `[release]`
- All tests pass

## Development Tips

### Local Testing

Test against local mock server:

```bash
# Start mock WebSocket server (you'll need to implement this)
# Then run:
KRAKEN_ENDPOINT=ws://localhost:8080 cargo run --example basic_usage
```

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo run --example basic_usage
```

### Performance Profiling

Profile with perf (Linux):

```bash
cargo build --release
perf record --call-graph=dwarf target/release/examples/basic_usage
perf report
```

### Memory Profiling

Use valgrind (Linux):

```bash
cargo build
valgrind --tool=memcheck target/debug/examples/basic_usage
```

## Troubleshooting

### Common Issues

1. **Build Failures**: Ensure Rust version is 1.70+
2. **Test Failures**: Check if all dependencies are installed
3. **Connection Issues**: Verify network connectivity and endpoints
4. **Performance Issues**: Run benchmarks to identify bottlenecks

### Getting Help

- Check existing issues on GitHub
- Review documentation: `cargo doc --open`
- Run examples to understand usage
- Join community discussions

## Architecture Decisions

### Design Principles

1. **Type Safety**: Leverage Rust's type system for compile-time safety
2. **Performance**: Minimize allocations and optimize hot paths
3. **Reliability**: Comprehensive error handling and recovery
4. **Usability**: Simple, intuitive API design
5. **Extensibility**: Modular architecture for easy extension

### Key Architectural Choices

- **Async/Await**: Non-blocking I/O for high performance
- **Arc/Mutex**: Thread-safe shared state management
- **Event-Driven**: Callback-based architecture for flexibility
- **Modular Design**: Separate concerns into focused modules
- **Error Handling**: Comprehensive error types with context

This development guide should help you contribute effectively to the Kraken WebSocket SDK. For questions or suggestions, please open an issue or start a discussion.