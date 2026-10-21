pub mod common;

const RULE: &str = "PDFUA1-DYNAMIC-XFA-001";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-15-1-dynamic-xfa.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-15-1-dynamic-xfa.pdf").to_vec(),
            || common::pdfua1_rule_7_15_1_fixture("dynamic_xfa"),
            &[
                "PDFUA1-DYNAMIC-XFA-001",
                "PDFUA1-METADATA-LANGUAGE-001",
            ],
        ),
        (
            "pdfua1-rule-7-15-1-no-xfa.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-15-1-no-xfa.pdf").to_vec(),
            || common::pdfua1_rule_7_15_1_fixture("no_xfa"),
            &["PDFUA1-METADATA-LANGUAGE-001"],
        ),
        (
            "pdfua1-rule-7-15-1-static-xfa.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-15-1-static-xfa.pdf").to_vec(),
            || common::pdfua1_rule_7_15_1_fixture("static_xfa"),
            &["PDFUA1-METADATA-LANGUAGE-001"],
        ),
    ],
}
