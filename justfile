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

# Run Clippy with the same strict settings as CI.
lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Run the complete offline test suite.
test:
    cargo test --workspace --all-features --locked

# Run formatting, linting, and offline tests.
check: fmt lint test

# Generate the crate documentation.
doc:
    cargo doc --workspace --all-features --locked --no-deps

# Regenerate the deterministic Typst 0.15.0 PDF/A-1b acceptance fixture.
typst:
    typst compile crates/tag_validation/tests/fixtures/typst-pdfa-1b.typ crates/tag_validation/tests/fixtures/typst-pdfa-1b.pdf --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

# Compare one PDF with pinned veraPDF; format may be text or json.
diff file format="text" verapdf=verapdf_bin:
    cargo run --quiet -p tag_cli --bin verapdf-diff -- --verapdf "{{ verapdf }}" --format {{ format }} "{{ file }}"

# Serve documentation
preview:
    uvx zensical serve

install:
    cargo install --path crates/tag_cli --force
