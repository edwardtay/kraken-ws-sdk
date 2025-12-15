#!/bin/bash

# Comprehensive test runner for Kraken WebSocket SDK

set -e

echo "🦑 Kraken WebSocket SDK - Comprehensive Test Suite"
echo "=================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    print_error "Cargo is not installed. Please install Rust and Cargo first."
    exit 1
fi

print_status "Rust version:"
rustc --version
cargo --version

# Clean previous builds
print_status "Cleaning previous builds..."
cargo clean

# Check code formatting
print_status "Checking code formatting..."
if cargo fmt --all -- --check; then
    print_success "Code formatting is correct"
else
    print_error "Code formatting issues found. Run 'cargo fmt' to fix."
    exit 1
fi

# Run clippy for linting
print_status "Running clippy lints..."
if cargo clippy --all-targets --all-features -- -D warnings; then
    print_success "No clippy warnings found"
else
    print_error "Clippy warnings found. Please fix them."
    exit 1
fi

# Build the project
print_status "Building the project..."
if cargo build --all-features; then
    print_success "Build completed successfully"
else
    print_error "Build failed"
    exit 1
fi

# Run unit tests
print_status "Running unit tests..."
if cargo test --test unit_tests --verbose; then
    print_success "Unit tests passed"
else
    print_error "Unit tests failed"
    exit 1
fi

# Run integration tests
print_status "Running integration tests..."
if cargo test --test integration_tests --verbose; then
    print_success "Integration tests passed"
else
    print_error "Integration tests failed"
    exit 1
fi

# Run property-based tests
print_status "Running property-based tests..."
if cargo test --test property_tests --verbose; then
    print_success "Property tests passed"
else
    print_error "Property tests failed"
    exit 1
fi

# Run parser tests
print_status "Running parser tests..."
if cargo test --test parser_tests --verbose; then
    print_success "Parser tests passed"
else
    print_error "Parser tests failed"
    exit 1
fi

# Run all library tests
print_status "Running library tests..."
if cargo test --lib --verbose; then
    print_success "Library tests passed"
else
    print_error "Library tests failed"
    exit 1
fi

# Build examples
print_status "Building examples..."
examples=("basic_usage" "advanced_usage" "kraken_live_test")

for example in "${examples[@]}"; do
    print_status "Building example: $example"
    if cargo build --example "$example"; then
        print_success "Example $example built successfully"
    else
        print_error "Failed to build example: $example"
        exit 1
    fi
done

# Run examples (basic tests only, not live connections)
print_status "Testing basic example..."
if timeout 10s cargo run --example basic_usage; then
    print_success "Basic example ran successfully"
else
    print_warning "Basic example timed out or failed (expected for demo)"
fi

# Run benchmarks if requested
if [[ "$1" == "--bench" ]]; then
    print_status "Running benchmarks..."
    if cargo bench --bench performance_benchmarks; then
        print_success "Benchmarks completed"
    else
        print_warning "Benchmarks failed or not available"
    fi
fi

# Generate documentation
print_status "Generating documentation..."
if cargo doc --no-deps --all-features; then
    print_success "Documentation generated successfully"
    print_status "Documentation available at: target/doc/kraken_ws_sdk/index.html"
else
    print_error "Documentation generation failed"
    exit 1
fi

# Security audit (if cargo-audit is installed)
if command -v cargo-audit &> /dev/null; then
    print_status "Running security audit..."
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues"
    fi
else
    print_warning "cargo-audit not installed. Run 'cargo install cargo-audit' for security checks."
fi

# Test coverage (if cargo-llvm-cov is installed)
if command -v cargo-llvm-cov &> /dev/null; then
    print_status "Generating test coverage report..."
    if cargo llvm-cov --all-features --workspace --html; then
        print_success "Coverage report generated at target/llvm-cov/html/index.html"
    else
        print_warning "Coverage report generation failed"
    fi
else
    print_warning "cargo-llvm-cov not installed. Run 'cargo install cargo-llvm-cov' for coverage reports."
fi

# Final summary
echo ""
echo "🎉 Test Suite Summary"
echo "===================="
print_success "✅ Code formatting"
print_success "✅ Clippy lints"
print_success "✅ Build"
print_success "✅ Unit tests"
print_success "✅ Integration tests"
print_success "✅ Property tests"
print_success "✅ Parser tests"
print_success "✅ Library tests"
print_success "✅ Examples build"
print_success "✅ Documentation"

echo ""
print_success "All tests completed successfully! 🚀"
print_status "Ready for production use."

# Optional: Run live test if environment is set up
if [[ -n "$KRAKEN_LIVE_TEST" ]]; then
    print_status "Running live connection test..."
    print_warning "This will attempt to connect to Kraken's WebSocket API"
    if timeout 30s cargo run --example kraken_live_test; then
        print_success "Live test completed"
    else
        print_warning "Live test timed out or failed"
    fi
fi