
build:
	anchor build

# Cleans everything so i can start with new program_id, used when i want to redeploy on clean state
# Before using this first use solana program close on the old one
rebuild:
	rm -rf target
	anchor keys sync
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
