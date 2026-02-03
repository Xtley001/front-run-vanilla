#!/bin/bash

# ============================================================
# FRONT RUN VANILLA - Complete Setup Script
# Optimized for GitHub Codespaces & Local Development
# ============================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Detect if running in Codespaces
if [ -n "$CODESPACES" ]; then
    IS_CODESPACES=true
    echo -e "${CYAN}Detected GitHub Codespaces environment${NC}"
else
    IS_CODESPACES=false
fi

echo -e "${BLUE}"
cat << "EOF"
╔════════════════════════════════════════════════╗
║    FRONT RUN VANILLA - HFT Trading System     ║
║         Setup & Installation Script           ║
╚════════════════════════════════════════════════╝
EOF
echo -e "${NC}"

# Check Rust installation
echo -e "${BLUE}[1/12]${NC} Checking Rust installation..."
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(cargo --version)
    echo -e "${GREEN}   ✓ ${RUST_VERSION}${NC}"
else
    echo -e "${YELLOW}   Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    echo -e "${GREEN}   ✓ Rust installed${NC}"
fi

# Update Rust
echo -e "${BLUE}[2/12]${NC} Updating Rust toolchain..."
rustup update stable
rustup component add clippy rustfmt
echo -e "${GREEN}   ✓ Toolchain updated${NC}"

# Install development tools
echo -e "${BLUE}[3/12]${NC} Installing development tools..."
if ! command -v cargo-watch &> /dev/null; then
    cargo install cargo-watch --quiet 2>&1 | tail -1
fi
echo -e "${GREEN}   ✓ Dev tools ready${NC}"

# Create directory structure
echo -e "${BLUE}[4/12]${NC} Creating directory structure..."
mkdir -p logs data/{orderbook,trades,funding} target
chmod 755 logs data
echo -e "${GREEN}   ✓ Directories created${NC}"

# Setup environment
echo -e "${BLUE}[5/12]${NC} Setting up environment..."
if [ ! -f ".env" ]; then
    cp .env.example .env
    echo -e "${YELLOW}   ⚠  Created .env - please configure API keys${NC}"
else
    echo -e "${GREEN}   ✓ .env exists${NC}"
fi

# Build project
echo -e "${BLUE}[6/12]${NC} Building project in release mode..."
echo -e "${CYAN}   This may take 3-5 minutes on first build...${NC}"
cargo build --release 2>&1 | grep -E "(Compiling|Finished|error)" | tail -20
if [ ${PIPESTATUS[0]} -eq 0 ]; then
    echo -e "${GREEN}   ✓ Build successful${NC}"
else
    echo -e "${RED}   ✗ Build failed - check errors above${NC}"
    exit 1
fi

# Run tests
echo -e "${BLUE}[7/12]${NC} Running test suite..."
TEST_OUTPUT=$(cargo test --release 2>&1)
TEST_RESULT=$?
if [ $TEST_RESULT -eq 0 ]; then
    PASSED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= passed)' | tail -1)
    echo -e "${GREEN}   ✓ All $PASSED tests passed${NC}"
else
    echo -e "${YELLOW}   ⚠  Some tests failed${NC}"
    echo "$TEST_OUTPUT" | grep -E "test result|FAILED" | tail -5
fi

# Run clippy
echo -e "${BLUE}[8/12]${NC} Running clippy (linter)..."
cargo clippy --quiet -- -D warnings 2>&1 | tail -5
if [ ${PIPESTATUS[0]} -eq 0 ]; then
    echo -e "${GREEN}   ✓ No lint warnings${NC}"
else
    echo -e "${YELLOW}   ⚠  Lint warnings found${NC}"
fi

# Check formatting
echo -e "${BLUE}[9/12]${NC} Checking code formatting..."
cargo fmt -- --check 2>&1 | tail -3
if [ ${PIPESTATUS[0]} -eq 0 ]; then
    echo -e "${GREEN}   ✓ Code properly formatted${NC}"
else
    echo -e "${YELLOW}   ⚠  Run 'cargo fmt' to format code${NC}"
fi

# Verify binaries
echo -e "${BLUE}[10/12]${NC} Verifying compiled binaries..."
BINARIES=("live_trader" "paper_trader" "backtester" "data_collector")
ALL_BINARIES_OK=true
for binary in "${BINARIES[@]}"; do
    if [ -f "target/release/$binary" ]; then
        SIZE=$(ls -lh "target/release/$binary" | awk '{print $5}')
        echo -e "${GREEN}   ✓ $binary ($SIZE)${NC}"
    else
        echo -e "${RED}   ✗ $binary not found${NC}"
        ALL_BINARIES_OK=false
    fi
done

# Run benchmarks (optional, only if requested)
if [ "$1" = "--with-bench" ]; then
    echo -e "${BLUE}[11/12]${NC} Running performance benchmarks..."
    echo -e "${CYAN}   This will take 2-3 minutes...${NC}"
    cargo bench --quiet 2>&1 | grep -E "(time:|Benchmarking)" | tail -10
    echo -e "${GREEN}   ✓ Benchmarks complete${NC}"
    echo -e "${CYAN}   View: target/criterion/report/index.html${NC}"
else
    echo -e "${BLUE}[11/12]${NC} Skipping benchmarks (use --with-bench to run)"
fi

# Final checks
echo -e "${BLUE}[12/12]${NC} Final verification..."

# Check if config files exist
CONFIG_OK=true
for config in production paper_trading backtest; do
    if [ -f "config/${config}.toml" ]; then
        echo -e "${GREEN}   ✓ config/${config}.toml${NC}"
    else
        echo -e "${RED}   ✗ config/${config}.toml missing${NC}"
        CONFIG_OK=false
    fi
done

# Create quick start guide
cat > QUICKSTART.txt << 'QUICKEOF'
FRONT RUN VANILLA - Quick Start

═══════════════════════════════════════════════════

COMMANDS (using Makefile):

  make paper       - Paper trading (safe, no real money)
  make backtest    - Run backtest
  make test        - Run tests
  make bench       - Run benchmarks
  make help        - Show all commands

OR using cargo directly:

  cargo run --release --bin paper_trader
  cargo run --release --bin backtester
  cargo test
  cargo bench

═══════════════════════════════════════════════════

NEXT STEPS:

1. Configure API keys:
   nano .env

2. Test with paper trading:
   make paper

3. Run backtests:
   make backtest

4. Read documentation:
   cat GREYPAPER.md
   cat docs/COMPLETE_GUIDE.md

═══════════════════════════════════════════════════

IMPORTANT:

⚠️  Do NOT run live_trader without:
   1. Testing in paper mode for 7+ days
   2. Successful backtests (Sharpe > 1.5)
   3. Using Binance TESTNET first
   4. Starting with small capital ($100-500)

═══════════════════════════════════════════════════
QUICKEOF

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║          Setup Complete! ✓                     ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
echo ""

# Display summary
echo -e "${MAGENTA}📊 System Summary:${NC}"
echo -e "   Rust: $(rustc --version | awk '{print $2}')"
echo -e "   Binaries: $(if $ALL_BINARIES_OK; then echo '✓ All compiled'; else echo '✗ Some missing'; fi)"
echo -e "   Tests: $(if [ $TEST_RESULT -eq 0 ]; then echo "✓ Passing"; else echo "⚠  Some failed"; fi)"
echo -e "   Config: $(if $CONFIG_OK; then echo '✓ Complete'; else echo '✗ Incomplete'; fi)"
echo ""

echo -e "${CYAN}🚀 Quick Start:${NC}"
echo ""
echo -e "   ${YELLOW}1. Configure API credentials:${NC}"
echo -e "      nano .env"
echo ""
echo -e "   ${YELLOW}2. Run paper trading:${NC}"
echo -e "      make paper"
echo -e "      ${CYAN}(or: cargo run --release --bin paper_trader)${NC}"
echo ""
echo -e "   ${YELLOW}3. Run tests:${NC}"
echo -e "      make test"
echo ""

if [ "$IS_CODESPACES" = true ]; then
    echo -e "${CYAN}📌 Codespaces Tips:${NC}"
    echo -e "   • Port 9090 will auto-forward (Prometheus metrics)"
    echo -e "   • Use VS Code terminal for best experience"
    echo -e "   • Rust Analyzer extension is pre-installed"
    echo ""
fi

echo -e "${GREEN}📖 Documentation:${NC}"
echo -e "   • GREYPAPER.md - Complete technical specification"
echo -e "   • docs/COMPLETE_GUIDE.md - Setup & usage"
echo -e "   • QUICKSTART.txt - Quick commands reference"
echo -e "   • README.md - Project overview"
echo ""

echo -e "${RED}⚠️  WARNING:${NC}"
echo -e "   This system trades REAL MONEY!"
echo -e "   • Test thoroughly before live trading"
echo -e "   • Always use risk limits"
echo -e "   • Start with testnet"
echo -e "   • Only risk capital you can afford to lose"
echo ""

echo -e "${GREEN}Happy Trading! 📈${NC}"
echo ""
