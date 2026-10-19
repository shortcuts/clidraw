.PHONY: build release run test lint fmt fmt-check perf check clean

build:
	cargo build

release:
	cargo build --release

run:
	cargo run

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

perf:
	cargo test --release --test perf -- --ignored

check: fmt-check lint test perf

clean:
	cargo clean
