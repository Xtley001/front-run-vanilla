#!/bin/bash

# ============================================================
# FRONT RUN VANILLA - Codespaces Post-Create Setup
# ============================================================

set -e

echo "🚀 Setting up Front Run Vanilla in GitHub Codespaces..."
echo ""

# Update Rust to latest stable
echo "📦 Updating Rust toolchain..."
rustup update stable
rustup default stable
rustup component add clippy rustfmt

# Install cargo tools for development
echo "🔧 Installing cargo tools..."
cargo install cargo-watch cargo-edit cargo-audit --quiet 2>/dev/null || true

# Create necessary directories
echo "📁 Creating directories..."
mkdir -p logs data/{orderbook,trades,funding}
chmod 755 logs data

# Setup environment file
if [ ! -f ".env" ]; then
    echo "📝 Creating .env file..."
    cp .env.example .env
fi

# Build project in release mode
echo "🔨 Building project (this may take 2-3 minutes)..."
cargo build --release 2>&1 | tail -5

# Run tests
echo "🧪 Running tests..."
cargo test --release --quiet 2>&1 | tail -10

# Make scripts executable
chmod +x scripts/*.sh 2>/dev/null || true
chmod +x setup.sh 2>/dev/null || true

echo ""
echo "✅ Setup complete!"
echo ""
echo "╔════════════════════════════════════════════════╗"
echo "║  Front Run Vanilla - Ready to Trade! 📈        ║"
echo "╚════════════════════════════════════════════════╝"
echo ""
echo "📚 Next steps:"
echo "  1. Edit .env with your Binance API keys"
echo "  2. Run paper trader: cargo run --bin paper_trader"
echo "  3. Run tests: cargo test"
echo "  4. View docs: cat README.md"
echo ""
echo "⚡ Quick commands:"
echo "  Paper trading:  cargo run --release --bin paper_trader"
echo "  Backtesting:    cargo run --release --bin backtester"
echo "  Run tests:      cargo test"
echo "  Benchmarks:     cargo bench"
echo ""
echo "📖 Documentation in docs/ folder"
echo "⚠️  Always use testnet before live trading!"
echo ""
