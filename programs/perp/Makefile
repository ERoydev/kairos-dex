
# Create symlink to idl files in scripts/idls to automate initialization of markets
build:
	anchor build
	ln -sf ../target/idl scripts/idls 

# Cleans everything so i can start with new program_id, used when i want to redeploy on clean state
# Before using this first use solana program close on the old one
rebuild:
	rm -rf target
	anchor keys sync
	anchor build

test:
	cargo build-sbf --features dev && cargo test --features dev

start:
	# watch auto-redeploys on recompile
	surfpool start --watch

integration-test:
	./test-integration.sh

# Execute surfpool runbooks in this order sequentially
deploy-devnet:
	surfpool run deployment --env devnet --unsupervised

init-global-devnet:
	surfpool run init-global --env devnet --unsupervised

init-market-devnet:
	surfpool run init-market --env devnet --unsupervised

open-position-devnet:
	npx tsx scripts/ts/open_position_normal.ts

# fee receiver must be initialized, before creating a position
initialize-fee-receiver:
	npx tsx scripts/ts/initialize_fee_receiver.ts

deposit-usdc-lp:
	npx tsx scripts/ts/deposit_usdc_in_lp.ts