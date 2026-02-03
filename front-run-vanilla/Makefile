# ============================================================
# FRONT RUN VANILLA - Makefile for GitHub Codespaces
# ============================================================

.PHONY: help build test bench paper backtest live clean format lint check watch install docs

# Default target
help:
	@echo "╔════════════════════════════════════════════════╗"
	@echo "║  Front Run Vanilla - Development Commands     ║"
	@echo "╚════════════════════════════════════════════════╝"
	@echo ""
	@echo "🔨 Building:"
	@echo "  make build       - Build in release mode"
	@echo "  make build-dev   - Build in debug mode"
	@echo ""
	@echo "🧪 Testing:"
	@echo "  make test        - Run all tests"
	@echo "  make bench       - Run benchmarks"
	@echo "  make check       - Run clippy checks"
	@echo "  make lint        - Run full lint"
	@echo ""
	@echo "📈 Trading:"
	@echo "  make paper       - Run paper trader (safe)"
	@echo "  make backtest    - Run backtester"
	@echo "  make live        - Run live trader (⚠️  REAL MONEY)"
	@echo ""
	@echo "🔧 Development:"
	@echo "  make watch       - Auto-rebuild on changes"
	@echo "  make format      - Format code"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make install     - Install dev tools"
	@echo ""
	@echo "📖 Documentation:"
	@echo "  make docs        - View documentation"
	@echo ""

# Build commands
build:
	@echo "🔨 Building in release mode..."
	@cargo build --release

build-dev:
	@echo "🔨 Building in debug mode..."
	@cargo build

# Testing
test:
	@echo "🧪 Running tests..."
	@cargo test --release

test-verbose:
	@echo "🧪 Running tests (verbose)..."
	@cargo test --release -- --nocapture

bench:
	@echo "⚡ Running benchmarks..."
	@cargo bench
	@echo "📊 View results: target/criterion/report/index.html"

# Code quality
check:
	@echo "🔍 Running clippy..."
	@cargo clippy -- -D warnings

lint:
	@echo "🔍 Running full lint..."
	@cargo clippy --all-targets --all-features -- -D warnings

format:
	@echo "✨ Formatting code..."
	@cargo fmt

format-check:
	@echo "🔍 Checking format..."
	@cargo fmt -- --check

# Trading binaries
paper:
	@echo "📈 Starting paper trader (no real money)..."
	@echo "⚠️  Press Ctrl+C to stop"
	@cargo run --release --bin paper_trader

backtest:
	@echo "📊 Running backtest..."
	@cargo run --release --bin backtester -- --start 2024-01-01 --end 2024-12-31

live:
	@echo "⚠️  WARNING: LIVE TRADING MODE - REAL MONEY!"
	@echo "Press Ctrl+C within 5 seconds to cancel..."
	@sleep 5
	@cargo run --release --bin live_trader

# Development tools
watch:
	@echo "👀 Watching for changes..."
	@cargo watch -x 'build --release'

watch-test:
	@echo "👀 Watching and testing..."
	@cargo watch -x test

install:
	@echo "📦 Installing development tools..."
	@cargo install cargo-watch cargo-edit cargo-audit

# Cleanup
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@rm -rf logs/*.log
	@rm -rf data/orderbook/* data/trades/* data/funding/*
	@echo "✓ Clean complete"

# Documentation
docs:
	@echo "📖 Opening documentation..."
	@cat README.md

# Setup
setup:
	@echo "🚀 Running setup..."
	@bash setup.sh

# Environment check
env-check:
	@echo "🔍 Checking environment..."
	@if [ -f .env ]; then \
		echo "✓ .env file exists"; \
	else \
		echo "✗ .env file missing - run: cp .env.example .env"; \
	fi
	@echo ""
	@echo "Rust version: $$(rustc --version)"
	@echo "Cargo version: $$(cargo --version)"

# Quick start
quickstart: env-check build test
	@echo ""
	@echo "╔════════════════════════════════════════════════╗"
	@echo "║  Ready to trade! Run: make paper               ║"
	@echo "╚════════════════════════════════════════════════╝"

# CI/CD simulation
ci: format-check lint test
	@echo "✓ CI checks passed!"

# All checks before commit
pre-commit: format lint test
	@echo "✓ Pre-commit checks passed!"
