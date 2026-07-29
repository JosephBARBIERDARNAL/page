# Export recipe parameters so the opt-in veraPDF test can receive its binary.

set export := true

# veraPDF is resolved from PATH unless the caller supplies VERAPDF_BIN.

verapdf_bin := env_var_or_default("VERAPDF_BIN", "verapdf")

# Show the available project commands.
default:
    @just --list

# Check formatting without changing files.
fmt:
    cargo fmt --all --check

# Format all Rust sources.
fmt-fix:
    cargo fmt --all

# Run Clippy with the same strict settings as CI.
lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Run the complete offline test suite.
test:
    cargo test --workspace --all-features --locked

# Run formatting, linting, and offline tests.
check: fmt lint test

# Compile the workspace in development mode.
build:
    cargo build --workspace --all-features --locked

# Compile optimized binaries.
build-release:
    cargo build --workspace --all-features --locked --release

# Generate the crate documentation.
doc:
    cargo doc --workspace --all-features --locked --no-deps

# Verify that committed PDF fixtures remain byte-exact.
test-fixtures:
    cargo test -p tag-validation --test fixture_integrity

# Run only the metadata atomic tests.
test-metadata:
    cargo test -p tag-validation --test metadata_atomic

# Run only the output-intent atomic tests.
test-output-intents:
    cargo test -p tag-validation --test output_intent_atomic

# Run only the CLI contract tests.
test-cli:
    cargo test -p tag-cli --test cli

# Compare all pinned cases with veraPDF from PATH.
test-verapdf VERAPDF_BIN=verapdf_bin:
    cargo test -p tag-validation --test verapdf_diff -- --nocapture

# Validate one PDF; format may be text or json.
validate file format="text":
    cargo run --quiet -p tag-cli --bin tag -- validate --profile pdfa-1b --format {{ format }} "{{ file }}"

# Compare one PDF with pinned veraPDF; format may be text or json.
diff file format="text" verapdf=verapdf_bin:
    cargo run --quiet -p tag-cli --bin verapdf-diff -- --verapdf "{{ verapdf }}" --format {{ format }} "{{ file }}"

# Remove Cargo build artifacts.
clean:
    cargo clean

# Serve documentation
preview:
    uvx zensical serve
