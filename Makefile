.PHONY: check test lint schema schema-check bench build run create_release
check:
	cargo check

test:
	cargo test

lint:
	cargo fmt --all -- --check
	cargo clippy -- -D warnings

schema:
	cargo run --quiet -- schema > schema/machine_setup.schema.json

schema-check: schema
	@git diff --exit-code -- schema/machine_setup.schema.json || \
		(echo "schema/machine_setup.schema.json is stale; run 'make schema' and commit" && exit 1)

bench:
	cargo bench --bench command_bench

build:
	cargo build --release

run:
	cargo run -- install -c ./example_config.yaml

create_release:
	./release/push_tag.sh
