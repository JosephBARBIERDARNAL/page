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

# Validate the pinned 129-predicate inventory and print its completion blockers.
coverage:
    cargo test -p page_validation --test coverage_inventory -- --nocapture

# Run every checked-in atomic and corpus case against pinned veraPDF.
verapdf verapdf=verapdf_bin:
    VERAPDF_BIN="{{ verapdf }}" cargo test -p page_validation --test verapdf_diff -- --nocapture

# Release-only gate. This intentionally fails while the inventory status is developing.
pdfa1b-release-gate verapdf=verapdf_bin:
    PAGE_REQUIRE_PDFA1B_COMPLETE=1 cargo test -p page_validation --test coverage_inventory -- --nocapture
    just verapdf "{{ verapdf }}"

# Regenerate deterministic Typst fixtures.
typst:
    typst compile crates/page_validation/tests/fixtures/not-compliant-1.typ --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/proposal.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/poster.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/resume.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/typst-pdfa-1b.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/typst-pdfa-1a.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

# Compare one PDF with pinned veraPDF; format may be text or json.
diff file format="text" verapdf=verapdf_bin:
    cargo run --quiet -p page_cli --bin verapdf-diff -- --verapdf "{{ verapdf }}" --format {{ format }} "{{ file }}"

# Build the release validator and compare it with veraPDF on the checked-in long PDF.
benchmark:
    typst compile bench/long-pdfa-1b.typ bench/long-pdfa-1b.pdf --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600
    cargo build --quiet --release -p page_cli --bin page
    rust-script bench/verapdf.rs

# Serve documentation
doc:
    uvx zensical serve

# Install locally
install:
    cargo install --path crates/page_cli --force
