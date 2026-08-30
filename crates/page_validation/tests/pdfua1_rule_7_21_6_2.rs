pub mod common;

const RULE: &str = "PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001";
const REFERENCE_RULE: &str = "ISO 14289-1:2014:7.21.6:2";

crate::pdfua1_rule_tests! {
    rule: RULE,
    reference_rule: REFERENCE_RULE,
    cases: [
        ("pdfua1-rule-7-21-6-2-invalid-differences.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-differences.pdf").to_vec(), || common::pdfua1_rule_7_21_6_2_fixture("invalid_differences"), &["PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001"], true, false, &[]),
        ("pdfua1-rule-7-21-6-2-invalid-encoding.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-invalid-encoding.pdf").to_vec(), || common::pdfua1_rule_7_21_6_2_fixture("invalid_encoding"), &["PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001"], true, false, &[]),
        ("pdfua1-rule-7-21-6-2-matching.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-matching.pdf").to_vec(), || common::pdfua1_rule_7_21_6_2_fixture("matching"), &[], false, false, &[]),
        ("pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf", || include_bytes!("fixtures/pdfua1-rule-7-21-6-2-missing-unicode-cmap.pdf").to_vec(), || common::pdfua1_rule_7_21_6_2_fixture("missing_unicode_cmap"), &["PDFUA1-TRUETYPE-NONSYMBOLIC-ENCODING-001"], true, false, &[]),
    ],
}
