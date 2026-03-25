.PHONY: build test run migrate clean lint fmt fmt-check install

build:
	cargo build --release

test:
	cargo test

lint:
	cargo clippy -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

install:
	cargo install --path .

migrate:
	sqlx migrate run

clean:
	cargo clean
	rm -f .aigit/db.sqlite