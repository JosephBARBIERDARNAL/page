# Export recipe parameters so veraPDF-backed tests can receive their binary.

set export := true

# veraPDF is resolved from PATH unless the caller supplies VERAPDF_BIN.

verapdf_bin := env_var_or_default("VERAPDF_BIN", "verapdf")
verapdf_corpus_repository := "https://github.com/veraPDF/veraPDF-corpus.git"
verapdf_corpus_revision := "49de56cd987929932c9e4fbbbe67d052bf44ef83"
verapdf_corpus_profiles := `awk '{ printf "%s ", $1 }' crates/page_cli/src/corpus_profiles.txt`

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

# Run every checked-in atomic and corpus case against pinned veraPDF.
verapdf verapdf=verapdf_bin:
    command -v "{{ verapdf }}" >/dev/null 2>&1 || { echo "veraPDF executable not found: {{ verapdf }}" >&2; exit 1; }
    VERAPDF_BIN="{{ verapdf }}" cargo test -p page_validation --test verapdf_diff -- --nocapture

# Run every validation test with the pinned veraPDF reference enabled.
verapdf-all verapdf=verapdf_bin:
    command -v "{{ verapdf }}" >/dev/null 2>&1 || { echo "veraPDF executable not found: {{ verapdf }}" >&2; exit 1; }
    VERAPDF_BIN="{{ verapdf }}" cargo test -p page_validation --tests --all-features --locked -- --nocapture

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
    cargo run --release -p page_cli --features internal --bin page-corpus -- "{{ corpus_dir }}"

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
    cargo build --quiet --release -p page_cli --bin page
    rust-script bench/verapdf.rs

# Regenerate checked-in rule-mapping documentation.
doc-rules:
    cargo test -p page_validation --test rule_mapping_docs regenerate_rule_mapping_documentation -- --ignored --exact

# Clean documentation cache
doc-clean:
    uvx zensical build --clean

# Serve documentation
preview:
    uvx zensical serve

# Build Rust reference documentation and put it in docs/api/rust/references/
doc-reference:
    uv run scripts/doc.py

# Install locally
install:
    cargo install --path crates/page_cli --force

# Recreate the logo files
logo:
    typst compile docs/images/logo.typ docs/images/logo-mark-on-dark.svg --input surface=dark --ppi 300
    typst compile docs/images/logo.typ docs/images/logo-mark-on-light.svg --input surface=light --ppi 300
    typst compile docs/images/logo.typ docs/images/logo-on-dark.svg --input label=false --input surface=dark --ppi 300
    typst compile docs/images/logo.typ docs/images/logo-on-light.svg --input label=false --input surface=light --ppi 300
