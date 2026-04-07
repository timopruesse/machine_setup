check:
	cargo check

test:
	cargo test

lint:
	cargo fmt --all -- --check
	cargo clippy -- -D warnings

build:
	cargo build --release

run:
	cargo run -- install -c ./example_config.yaml

create_release:
	./release/push_tag.sh
