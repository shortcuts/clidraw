.PHONY: build release run test lint fmt fmt-check perf profile check clean

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
	cargo test --release --test perf -- --ignored --nocapture --test-threads=1

profile:
	CARGO_PROFILE_RELEASE_DEBUG=1 cargo test --release --test perf --no-run
	samply record $$(CARGO_PROFILE_RELEASE_DEBUG=1 cargo test --release --test perf --no-run --message-format=json | grep -o '"executable":"[^"]*deps/perf-[^"]*"' | cut -d'"' -f4) drag_step_fits_the_budget_on_5000_elements --ignored --nocapture --test-threads=1

check: fmt-check lint test perf

clean:
	cargo clean
