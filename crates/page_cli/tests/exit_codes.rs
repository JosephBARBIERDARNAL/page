use page_cli::validation_error_exit_code;
use page_validation::{PdfError, ValidationError};

#[test]
fn validation_errors_preserve_cli_exit_codes() {
    let cases = [
        (
            ValidationError::InputIo(std::io::Error::other("read failed")),
            1,
        ),
        (
            ValidationError::Pdf(PdfError::Parse(Box::new(std::io::Error::other(
                "invalid PDF",
            )))),
            2,
        ),
        (
            ValidationError::Pdf(PdfError::InputTooLarge {
                actual: 2,
                limit: 1,
            }),
            1,
        ),
        (ValidationError::MissingProfileDeclaration, 1),
        (
            ValidationError::InvalidProfileDeclaration("invalid profile".to_owned()),
            1,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(validation_error_exit_code(&error), expected, "{error}");
    }
}
