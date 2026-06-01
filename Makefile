.PHONY: run build test docker-up docker-down

run:
	cargo run -p symphony-bridge --release

build:
	cargo build --release -p symphony-bridge

test:
	cargo test -p symphony-bridge

docker-up:
	docker compose up -d --build

docker-down:
	docker compose down
