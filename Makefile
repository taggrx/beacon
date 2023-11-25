start:
	dfx start --background -qqqq

deploy_local:
	dfx deploy

reinstall:
	make fe
	dfx deploy --mode=reinstall barebones -y

build:
	NODE_ENV=production make fe
	./build.sh barebones

test:
	cargo clippy --tests --benches -- -D clippy::all
	cargo test

fe:
	npm run build --quiet

release:
	docker build -t barebones .
	docker run --rm -v $(shell pwd)/release-artifacts:/target/wasm32-unknown-unknown/release barebones
	make hashes

hashes:
	git rev-parse HEAD
	shasum -a 256 ./release-artifacts/barebones.wasm.gz  | cut -d ' ' -f 1
