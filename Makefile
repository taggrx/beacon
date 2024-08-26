start:
	dfx start --background -qqqq

mainnet_deploy:
	make build
	dfx --identity prod deploy --network ic

local_deploy:
	FEATURES=dev dfx deploy

local_reinstall:
	make fe
	FEATURES=dev dfx deploy --mode=reinstall beacon -y

build:
	NODE_ENV=production make fe
	./build.sh beacon

test:
	cargo clippy --tests --benches -- -D clippy::all
	cargo test

fe:
	npm run build --quiet

release:
	docker build -t beacon .
	docker run --rm -v $(shell pwd)/release-artifacts:/target/wasm32-unknown-unknown/release beacon
	make hashes

podman_release:
	podman build -t beacon .
	podman run --rm -v $(shell pwd)/release-artifacts:/target/wasm32-unknown-unknown/release beacon
	make hashes

hashes:
	git rev-parse HEAD
	shasum -a 256 ./release-artifacts/beacon.wasm.gz  | cut -d ' ' -f 1
