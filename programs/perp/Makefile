
# Create symlink to idl files in scripts/idls to automate initialization of markets
build:
	anchor build
	ln -sf ../target/idl scripts/idls 

test:
	cargo build-sbf --features dev && cargo test --features dev
