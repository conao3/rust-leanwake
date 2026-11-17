.PHONY: build
build:
	$(MAKE) -C native build
	cargo build -j 4

.PHONY: check
check: build
	nix fmt -- --ci
	python3 tools/check-format.py
	cargo clippy --all-targets -j 4 -- -D warnings
	python3 tools/check-rules.py
	cargo test -j 4 -- --test-threads=1
	nix build --option cores 4 --max-jobs 1
	python3 tools/check-package.py result/bin/leanwake

.PHONY: format
format:
	nix fmt
