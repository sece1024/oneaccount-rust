APP        := oneaccount
VERSION    := $(shell cargo metadata --no-deps --format-version 1 | python3 -c "import sys,json; print(json.load(sys.stdin)['packages'][0]['version'])")
DIST       := dist

# ── 本机构建 ───────────────────────────────────────────────────────────────────

.PHONY: build
build:
	cargo build --release
	@echo "✅ 本机构建完成: target/release/$(APP)"

.PHONY: run-tui
run-tui:
	cargo run -- tui

.PHONY: run-server
run-server:
	cargo run -- server --port 8080

# ── 测试 ───────────────────────────────────────────────────────────────────────

.PHONY: test
test:
	cargo test

.PHONY: test-verbose
test-verbose:
	cargo test -- --nocapture

# ── 多平台交叉编译（需要安装 cross: cargo install cross）─────────────────────

$(DIST):
	mkdir -p $(DIST)

# macOS Apple Silicon
.PHONY: build-macos-arm
build-macos-arm: $(DIST)
	cross build --release --target aarch64-apple-darwin
	cp target/aarch64-apple-darwin/release/$(APP) $(DIST)/$(APP)-$(VERSION)-macos-arm64

# macOS Intel
.PHONY: build-macos-x86
build-macos-x86: $(DIST)
	cross build --release --target x86_64-apple-darwin
	cp target/x86_64-apple-darwin/release/$(APP) $(DIST)/$(APP)-$(VERSION)-macos-x86_64

# Windows x64（需要 x86_64-pc-windows-gnu 工具链）
.PHONY: build-windows
build-windows: $(DIST)
	cross build --release --target x86_64-pc-windows-gnu
	cp target/x86_64-pc-windows-gnu/release/$(APP).exe $(DIST)/$(APP)-$(VERSION)-windows-x86_64.exe

# Linux x64
.PHONY: build-linux-x86
build-linux-x86: $(DIST)
	cross build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(APP) $(DIST)/$(APP)-$(VERSION)-linux-x86_64

# 树莓派 64-bit（Pi 3B+/4/5，运行 64-bit OS）
.PHONY: build-rpi-arm64
build-rpi-arm64: $(DIST)
	cross build --release --target aarch64-unknown-linux-gnu
	cp target/aarch64-unknown-linux-gnu/release/$(APP) $(DIST)/$(APP)-$(VERSION)-linux-arm64

# 树莓派 32-bit（Pi Zero / 1 / 2）
.PHONY: build-rpi-armv7
build-rpi-armv7: $(DIST)
	cross build --release --target armv7-unknown-linux-gnueabihf
	cp target/armv7-unknown-linux-gnueabihf/release/$(APP) $(DIST)/$(APP)-$(VERSION)-linux-armv7

# 全平台构建
.PHONY: build-all
build-all: build-linux-x86 build-rpi-arm64 build-rpi-armv7 build-windows
	@echo "✅ 所有平台构建完成，输出目录: $(DIST)/"
	@ls -lh $(DIST)/

# ── 安装（当前平台）──────────────────────────────────────────────────────────

.PHONY: install
install: build
	cargo install --path .
	@echo "✅ 已安装到 ~/.cargo/bin/$(APP)"

# ── 安装 cross 工具 ──────────────────────────────────────────────────────────

.PHONY: install-cross
install-cross:
	cargo install cross --git https://github.com/cross-rs/cross
	@echo "✅ cross 已安装，需要 Docker 运行环境"

# ── 添加 Rust 目标平台 ────────────────────────────────────────────────────────

.PHONY: add-targets
add-targets:
	rustup target add aarch64-unknown-linux-gnu
	rustup target add armv7-unknown-linux-gnueabihf
	rustup target add x86_64-unknown-linux-gnu
	rustup target add x86_64-pc-windows-gnu
	@echo "✅ 目标平台已添加"

# ── 清理 ──────────────────────────────────────────────────────────────────────

.PHONY: clean
clean:
	cargo clean
	rm -rf $(DIST)

.PHONY: help
help:
	@echo "OneAccount 构建工具"
	@echo ""
	@echo "用法:"
	@echo "  make build           本机构建（release）"
	@echo "  make test            运行所有测试"
	@echo "  make run-tui         启动 TUI 界面"
	@echo "  make run-server      启动 HTTP 服务"
	@echo "  make build-rpi-arm64 交叉编译到树莓派 64-bit"
	@echo "  make build-rpi-armv7 交叉编译到树莓派 32-bit"
	@echo "  make build-all       编译所有平台"
	@echo "  make install-cross   安装 cross 工具（需要 Docker）"
	@echo "  make add-targets     添加 Rust 交叉编译目标"
	@echo "  make clean           清理构建产物"
