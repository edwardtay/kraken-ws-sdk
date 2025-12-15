#!/bin/bash

# Kraken WebSocket SDK - Web Demo Launcher

set -e

echo "🦑 Kraken WebSocket SDK - Web Demo Launcher"
echo "==========================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo is not installed. Please install Rust and Cargo first."
    exit 1
fi

# Navigate to web demo directory
if [ ! -d "examples/web_demo" ]; then
    echo "❌ Web demo directory not found. Please run this script from the project root."
    exit 1
fi

print_info "Building web demo application..."
cd examples/web_demo

# Build the application
if cargo build; then
    print_success "Build completed successfully"
else
    echo "❌ Build failed"
    exit 1
fi

print_info "Starting web demo server..."
print_info "Dashboard will be available at: http://localhost:3030"
print_info "Press Ctrl+C to stop the server"

echo ""
print_success "🌐 Starting Kraken WebSocket SDK Web Demo..."
echo ""

# Run the web demo
cargo run