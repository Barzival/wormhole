.PHONY: all
all: node_modules build nearcore

.PHONY: build
build: contracts/wormhole/target/wasm32-unknown-unknown/release/wormhole.wasm contracts/portal/target/wasm32-unknown-unknown/release/portal.wasm contracts/ft/target/wasm32-unknown-unknown/release/ft.wasm contracts/mock-bridge-integration/target/wasm32-unknown-unknown/release/mock_bridge_integration.wasm

contracts/wormhole/target/wasm32-unknown-unknown/release/wormhole.wasm: contracts/wormhole/src/*.rs contracts/wormhole/Cargo.toml
	cd contracts/wormhole; cargo build --target wasm32-unknown-unknown --release

contracts/portal/target/wasm32-unknown-unknown/release/portal.wasm: contracts/portal/src/*.rs  contracts/portal/Cargo.toml contracts/ft/target/wasm32-unknown-unknown/release/ft.wasm
	cd contracts/portal; cargo build --target wasm32-unknown-unknown --release

contracts/ft/target/wasm32-unknown-unknown/release/ft.wasm: contracts/ft/src/*.rs  contracts/ft/Cargo.toml
	cd contracts/ft; cargo build --target wasm32-unknown-unknown --release

contracts/mock-bridge-integration/target/wasm32-unknown-unknown/release/mock_bridge_integration.wasm: contracts/mock-bridge-integration/src/*.rs  contracts/mock-bridge-integration/Cargo.toml
	cd contracts/mock-bridge-integration; cargo build --target wasm32-unknown-unknown --release

package-lock.json: package.json
	npm install

node_modules: package-lock.json
	touch -m node_modules
	npm ci

nearcore:
	mkdir $@ && \
	cd $@ && \
	git init && \
	git remote add origin https://github.com/near/nearcore && \
	git fetch --depth 1 origin c6eb78ab11d0fb7c7bb9dfa6d712aba449a0140b && \
	git checkout FETCH_HEAD
	cd $@ && make sandbox-release

run: nearcore
	./start_node.sh

.PHONY: test
test: build node_modules
	npx ts-node test/test.ts

.PHONY: clean
clean:
	rm -rf nearcore \
		node_modules \
		contracts/ft/target/wasm32-unknown-unknown/release/ft.wasm \
		contracts/portal/target/wasm32-unknown-unknown/release/portal.wasm \
		contracts/wormhole/target/wasm32-unknown-unknown/release/wormhole.wasm \
                contracts/mock-bridge-integration/target/wasm32-unknown-unknown/release/mock-bridge-integration.wasm
