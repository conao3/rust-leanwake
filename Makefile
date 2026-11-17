.PHONY: build
build:
	$(MAKE) -C native build
	cargo build -j 4

.PHONY: check
check: build
	nix fmt -- --ci
	cargo clippy --all-targets -j 4 -- -D warnings
	python3 tools/check-rules.py
	cargo test -j 4 -- --test-threads=1

.PHONY: format
format:
	nix fmt
