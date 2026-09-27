pub mod common;

const RULE: &str = "PDFUA1-TYPE0-CID-SYSTEM-INFO-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-21-3-1-identity.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-identity.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("identity"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-1-matching.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-matching.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("matching"),
            &[],
        ),
        (
            "pdfua1-rule-7-21-3-1-registry-mismatch.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-registry-mismatch.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("registry_mismatch"),
            &["PDFUA1-TYPE0-CID-SYSTEM-INFO-001"],
        ),
        (
            "pdfua1-rule-7-21-3-1-supplement-mismatch.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-21-3-1-supplement-mismatch.pdf").to_vec(),
            || common::pdfua1_rule_7_21_3_1_fixture("supplement_mismatch"),
            &["PDFUA1-TYPE0-CID-SYSTEM-INFO-001"],
        ),
    ],
}
