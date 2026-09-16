
build:
	anchor build

build-devnet:
	anchor build -- --features dev

test:
	cargo build-sbf --features dev && cargo test --features dev

# Execute these surfpool runbooks sequantially in this order
deploy-devnet:
	surfpool run deployment --env devnet --unsupervised

init-pool-devnet:
	surfpool run init-pool --env devnet --unsupervised
