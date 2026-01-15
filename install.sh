#!/bin/sh
# Ratchet installation script
# Usage: curl -sSf https://raw.githubusercontent.com/imbue-ai/ratchet/main/install.sh | sh

set -e

# Colors for output (POSIX-compatible)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Helper functions for colored output
print_info() {
    printf "${BLUE}==>${NC} %s\n" "$1"
}

print_success() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

print_warning() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

print_error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
}

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux*)  OS="linux" ;;
        Darwin*) OS="macos" ;;
        *)
            print_error "Unsupported operating system: $OS"
            print_error "Ratchet currently supports Linux and macOS only."
            exit 1
            ;;
    esac

    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        arm64|aarch64) ARCH="aarch64" ;;
        *)
            print_error "Unsupported architecture: $ARCH"
            print_error "Ratchet currently supports x86_64 and aarch64/arm64 only."
            exit 1
            ;;
    esac

    print_info "Detected platform: $OS ($ARCH)"
}

# Check for required dependencies
check_dependencies() {
    print_info "Checking dependencies..."

    if ! command -v cargo >/dev/null 2>&1; then
        print_error "Rust and Cargo are required but not installed."
        printf "\nPlease install Rust from: https://rustup.rs/\n"
        printf "Then run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n\n"
        exit 1
    fi

    if ! command -v rustc >/dev/null 2>&1; then
        print_error "Rust compiler (rustc) is required but not found."
        printf "\nPlease ensure Rust is properly installed: https://rustup.rs/\n\n"
        exit 1
    fi

    if ! command -v git >/dev/null 2>&1; then
        print_error "Git is required but not installed."
        printf "\nPlease install Git and try again.\n\n"
        exit 1
    fi

    CARGO_VERSION="$(cargo --version | cut -d' ' -f2)"
    RUSTC_VERSION="$(rustc --version | cut -d' ' -f2)"
    print_success "Found Cargo $CARGO_VERSION and Rust $RUSTC_VERSION"
}

# Create temporary directory
create_temp_dir() {
    TEMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'ratchet-install')"
    print_info "Using temporary directory: $TEMP_DIR"
}

# Clone repository
clone_repo() {
    print_info "Cloning ratchet repository..."

    if ! git clone --depth 1 https://github.com/imbue-ai/ratchet.git "$TEMP_DIR/ratchet" >/dev/null 2>&1; then
        print_error "Failed to clone repository"
        exit 1
    fi

    print_success "Repository cloned successfully"
}

# Build the project
build_project() {
    print_info "Building ratchet from source (this may take a few minutes)..."

    cd "$TEMP_DIR/ratchet"

    if ! cargo build --release 2>&1 | grep -E '(Compiling|Finished|error|warning:)'; then
        print_error "Build failed"
        exit 1
    fi

    if [ ! -f "target/release/ratchet" ]; then
        print_error "Build succeeded but binary not found at expected location"
        exit 1
    fi

    print_success "Build completed successfully"
}

# Install binary
install_binary() {
    print_info "Installing ratchet to ~/.cargo/bin..."

    # Ensure ~/.cargo/bin exists
    mkdir -p "$HOME/.cargo/bin"

    # Copy binary
    cp "$TEMP_DIR/ratchet/target/release/ratchet" "$HOME/.cargo/bin/ratchet"
    chmod +x "$HOME/.cargo/bin/ratchet"

    print_success "Binary installed to ~/.cargo/bin/ratchet"
}

# Cleanup temporary directory
cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        print_info "Cleaning up temporary files..."
        rm -rf "$TEMP_DIR"
    fi
}

# Check if ratchet is in PATH
check_path() {
    if ! echo "$PATH" | grep -q "$HOME/.cargo/bin"; then
        print_warning "~/.cargo/bin is not in your PATH"
        printf "\nAdd the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):\n"
        printf "    export PATH=\"\$HOME/.cargo/bin:\$PATH\"\n\n"
        printf "Then reload your shell or run: source ~/.bashrc\n\n"
    fi
}

# Main installation flow
main() {
    printf "\n"
    print_info "Ratchet Installation Script"
    printf "\n"

    detect_platform
    check_dependencies
    create_temp_dir

    # Ensure cleanup happens on exit
    trap cleanup EXIT

    clone_repo
    build_project
    install_binary

    printf "\n"
    print_success "Ratchet has been installed successfully!"
    printf "\n"

    check_path

    # Test installation
    if command -v ratchet >/dev/null 2>&1; then
        INSTALLED_VERSION="$(ratchet --version 2>/dev/null || echo 'unknown')"
        print_success "Verified installation: $INSTALLED_VERSION"
        printf "\nGet started by running:\n"
        printf "    ratchet init\n\n"
    else
        print_warning "Installation complete but 'ratchet' command not found in PATH"
        printf "You may need to add ~/.cargo/bin to your PATH and reload your shell.\n\n"
    fi
}

# Run main installation
main
