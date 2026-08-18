# Export recipe parameters so the opt-in veraPDF test can receive its binary.

set export := true

# veraPDF is resolved from PATH unless the caller supplies VERAPDF_BIN.

verapdf_bin := env_var_or_default("VERAPDF_BIN", "verapdf")
verapdf_corpus_repository := "https://github.com/veraPDF/veraPDF-corpus.git"
verapdf_corpus_revision := "49de56cd987929932c9e4fbbbe67d052bf44ef83"
verapdf_corpus_profiles := "PDF_A-1a PDF_A-1b PDF_A-2a PDF_A-2b PDF_A-2u PDF_A-3b"

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

# Run formatting and linting
check: fmt lint

# Validate the pinned 129-predicate inventory and print its completion blockers.
coverage:
    cargo test -p page_validation --test coverage_inventory -- --nocapture

# Run every checked-in atomic and corpus case against pinned veraPDF.
verapdf verapdf=verapdf_bin:
    VERAPDF_BIN="{{ verapdf }}" cargo test -p page_validation --test verapdf_diff -- --nocapture

# Run the opt-in differential suites for every implemented PDF/A-1, PDF/A-2, and PDF/A-3 profile.
verapdf-all verapdf=verapdf_bin:
    VERAPDF_BIN="{{ verapdf }}" cargo test -p page_validation --test canonical_compliance --test verapdf_diff --test pdfa_2_3_differential -- --nocapture

# Run page's expected-result gate over the selected profiles in a veraPDF corpus checkout.
verapdf-corpus corpus_dir=".cache/verapdf-corpus":
    if ! test -d "{{ corpus_dir }}"; then \
        mkdir -p "{{ corpus_dir }}"; \
        git -C "{{ corpus_dir }}" init --quiet; \
        git -C "{{ corpus_dir }}" remote add origin "{{ verapdf_corpus_repository }}"; \
        git -C "{{ corpus_dir }}" fetch --quiet --filter=blob:none --depth=1 origin "{{ verapdf_corpus_revision }}"; \
        git -C "{{ corpus_dir }}" sparse-checkout set --no-cone {{ verapdf_corpus_profiles }}; \
        git -C "{{ corpus_dir }}" checkout --detach --quiet FETCH_HEAD; \
    fi
    cargo run --quiet --release -p page_cli --bin page -- corpus "{{ corpus_dir }}"

# Regenerate checked-in rule-mapping documentation.
rules-docs:
    cargo test -p page_validation --test rule_mapping_docs regenerate_rule_mapping_documentation -- --ignored --exact

# Release-only gate for every currently implemented PDF/A-1, PDF/A-2, and PDF/A-3 profile.
pdfa-release-gate verapdf=verapdf_bin:
    PAGE_REQUIRE_PDFA1B_COMPLETE=1 cargo test -p page_validation --test coverage_inventory -- --nocapture
    PAGE_REQUIRE_PDFA23_COMPLETE=1 cargo test -p page_validation --test pdfa_2_3_differential pdfa_2_and_3_release_gate_requires_completed_inventory -- --nocapture
    cargo test -p page_validation --test rule_mapping_docs --test canonical_compliance
    just verapdf-all "{{ verapdf }}"

# Regenerate deterministic Typst fixtures.
typst:
    typst compile crates/page_validation/tests/fixtures/not-compliant-1.typ --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/proposal.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/poster.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/resume.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/typst-pdfa-1b.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/typst-pdfa-1a.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1b.typ --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1a.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1a-fonts.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1a-structure.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1a-content.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

    typst compile crates/page_validation/tests/fixtures/canonical-pdfa-1a-annotations.typ --pdf-standard a-1a --ignore-system-fonts --creation-timestamp 1767225600

# Compare one PDF with pinned veraPDF; format may be text or json.
diff file format="text" verapdf=verapdf_bin:
    cargo run --quiet -p page_cli --bin verapdf-diff -- --verapdf "{{ verapdf }}" --format {{ format }} "{{ file }}"

# Build the release validator and compare it with veraPDF on the checked-in long PDF.
benchmark:
    typst compile bench/long-pdfa-1b.typ bench/long-pdfa-1b.pdf --pdf-standard a-1b --ignore-system-fonts --creation-timestamp 1767225600
    cargo build --quiet --release -p page_cli --bin page
    rust-script bench/verapdf.rs

# Build documentation after regenerating rule mappings.
doc-build: rules-docs
    uvx zensical build --clean

# Serve documentation after regenerating rule mappings.
doc: rules-docs
    uvx zensical serve

# Install locally
install:
    cargo install --path crates/page_cli --force
