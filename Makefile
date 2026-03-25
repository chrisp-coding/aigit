.PHONY: build test run migrate clean

build:
	cargo build --release

test:
	cargo test

run:
	cargo run -- $(filter-out $@,$(MAKECMDGOALS))

migrate:
	sqlx migrate run

clean:
	cargo clean
	rm -f .aigit/db.sqlite

%:
	@: