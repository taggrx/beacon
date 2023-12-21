start:
	dfx start --background -qqqq

deploy:
	make build
	dfx --identity prod deploy --network ic

deploy_local:
	dfx deploy

reinstall:
	make fe
	dfx deploy --mode=reinstall beacon -y

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

hashes:
	git rev-parse HEAD
	shasum -a 256 ./release-artifacts/beacon.wasm.gz  | cut -d ' ' -f 1
