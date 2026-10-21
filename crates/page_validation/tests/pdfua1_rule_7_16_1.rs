pub mod common;

use page_validation::{
    PageError, PdfError, ValidationOptions, ValidationProfile, validate_pdf_bytes,
};

const RULE: &str = "ISO 14289-1:2014:7.16:1";

crate::pdfua1_rule_tests! {
    rule: RULE,
    cases: [
        (
            "pdfua1-rule-7-16-1-bit-10-false.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-16-1-bit-10-false.pdf").to_vec(),
            || common::pdfua1_rule_7_16_1_fixture("bit_10_false"),
            &["ISO 14289-1:2014:7.16:1", "ISO 14289-1:2014:7.2:33"],
        ),
        (
            "pdfua1-rule-7-16-1-valid.pdf",
            || include_bytes!("fixtures/pdfua1-rule-7-16-1-valid.pdf").to_vec(),
            || common::pdfua1_rule_7_16_1_fixture("valid"),
            &["ISO 14289-1:2014:7.2:33"],
        ),
    ],
}

#[test]
fn encrypted_pdf_with_an_invalid_encryption_dictionary_is_a_parser_error() {
    let error = validate_pdf_bytes(
        include_bytes!("fixtures/pdfua1-rule-7-16-1-missing-p.pdf"),
        &ValidationOptions::default().profile(ValidationProfile::PdfUa1),
    )
    .expect_err("veraPDF cannot parse the encrypted fixture without a password");

    assert!(matches!(error, PageError::Pdf(PdfError::InvalidCatalog)));
    assert_eq!(error.rule_id(), "PDF-PARSE-001");
}
