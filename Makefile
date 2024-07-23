ARGUMENTS = $(filter-out $@,$(MAKECMDGOALS))
GET_ARGUMENT = $(strip $(call word,$(1),$(ARGUMENTS)))

# Add this at the beginning of your Makefile
.PHONY: help
help:
	@echo "Available commands:"
	@echo
	@echo "Build:"
	@echo "  build                   - Build in release mode"
	@echo "  build-server-watch      - Watch and build with debug logging"
	@echo
	@echo "Changelog:"
	@echo "  changelog-update        - Update CHANGELOG.md with changes between the last two release commits"
	@echo
	@echo "Clean:"
	@echo "  clean                   - Remove Cargo.lock and clean the build artifacts"
	@echo
	@echo "Distribution:"
	@echo "  install-cargo-dist      - Install cargo-dist"
	@echo "  dist-plan               - Plan distribution"
	@echo
	@echo "Formatting:"
	@echo "  fmt                     - Format all code"
	@echo
	@echo "Linting:"
	@echo "  lint                    - Run clippy on all targets"
	@echo
	@echo "Tagging:"
	@echo "  tag                     - Create a new version tag"
	@echo "  tag-delete              - Delete a version tag"
	@echo "  tag-rollback            - Rollback a version tag"
	@echo "Testing:"
	@echo "  test                    - Run tests"
	@echo "  test-watch              - Watch and run tests"
	@echo
	@echo "For more details on each command, check the Makefile"

# Build

build:
	cargo build --release

build-watch:
	cargo watch -c --ignore .dbs -x check -x clippy --shell "RUST_LOG=debug cargo build"

# Changelog

changelog-update:
	@commits=$$(git log --grep="release: version" --format="%H" -n 2); \
	if [ $$(echo "$$commits" | wc -l) -lt 2 ]; then \
		echo "Error: Less than two 'release' commits found."; \
		exit 1; \
	fi; \
	last=$$(echo "$$commits" | head -n 1); \
	prev=$$(echo "$$commits" | tail -n 1); \
	git cliff $$prev..$$last --prepend CHANGELOG.md; \
	echo "CHANGELOG.md has been updated with changes between commits:"; \
	echo "Previous: $$prev"; \
	echo "Latest: $$last"

# Clean

clean:
	rm -rf Cargo.lock | cargo clean

# Dist

install-cargo-dist:
	cargo install cargo-dist --locked

dist-plan:
	cargo dist plan

# Format

fmt:
	cargo fmt --all -- --check

# Lint

lint:
	cargo clippy --all-targets --all-features --workspace

# Tag

tag:
	perl -pi -e 's/version = "$(call GET_ARGUMENT,1)"/version = "$(call GET_ARGUMENT,2)"/g' ./Cargo.toml
	@if [ "$(findstring prerelease,$(call GET_ARGUMENT,2))" = "prerelease" ]; then \
		perl -pi -e 's/targets = \["aarch64\-apple\-darwin", "x86_64\-apple\-darwin", "x86_64\-unknown\-linux\-gnu", "x86_64\-pc\-windows\-msvc"\]/targets = \["x86_64\-unknown\-linux\-gnu"\]/g' ./Cargo.toml; \
    fi
	cargo check --workspace
	git add Cargo.lock
	git add Cargo.toml
	git commit -m "release: version $(call GET_ARGUMENT,2)"
	git push --force-with-lease
	git tag v$(call GET_ARGUMENT,2)
	git push --tags

tag-rollback:
	@read -p "Are you sure you want to rollback the tag version $(ARGUMENTS)? [Y/n] " REPLY; \
    if [ "$$REPLY" = "Y" ] || [ "$$REPLY" = "y" ] || [ "$$REPLY" = "" ]; then \
        git reset --soft HEAD~1; \
		git reset HEAD Cargo.lock; \
		git reset HEAD Cargo.toml; \
		git checkout -- Cargo.lock; \
		git checkout -- Cargo.toml; \
		git tag -d v$(ARGUMENTS); \
		git push origin --delete v$(ARGUMENTS); \
		git push --force-with-lease; \
    else \
        echo "Aborted."; \
    fi

# Test

test:
	cargo test

test-watch:
	cargo watch -c -s "make test"

# catch anything and do nothing
%:
	@:
