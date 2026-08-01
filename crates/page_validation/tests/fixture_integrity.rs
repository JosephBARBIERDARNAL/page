use sha2::{Digest, Sha256};

#[test]
fn pdf_fixtures_remain_byte_exact() {
    let fixtures: [(&str, &[u8], &str); 9] = [
        (
            "encrypted.pdf",
            include_bytes!("fixtures/encrypted.pdf"),
            "59f52bf64091ea195ad0315510a724911ae74daa7a916c3c2d68d96a4150e819",
        ),
        (
            "example-1.pdf",
            include_bytes!("fixtures/example-1.pdf"),
            "44551a368f809db398644df056731d0f8acd2fd4ca03ef6f217646cf5c6eab56",
        ),
        (
            "malformed.pdf",
            include_bytes!("fixtures/malformed.pdf"),
            "df8088e6de6e266de2bcf72a2d1c9e8eee38b393c651184826be82da2877b65d",
        ),
        (
            "not-compliant-1.pdf",
            include_bytes!("fixtures/not-compliant-1.pdf"),
            "5bc593d878564406d81995b39ff0d0e578eb850b9cc05b5f406a8b80b870e61a",
        ),
        (
            "poster.pdf",
            include_bytes!("fixtures/poster.pdf"),
            "9412e2b32a45b9519b8edc15ef4c172b51105bf4b9917a4c2210ac422fbeb39c",
        ),
        (
            "proposal.pdf",
            include_bytes!("fixtures/proposal.pdf"),
            "e3cf5632ea869f210697c196a05a270c87474e9f99eece78cd91d2ab714b361c",
        ),
        (
            "resume.pdf",
            include_bytes!("fixtures/resume.pdf"),
            "816409ba5c7099973c14ad16b75f0f3a22ff62ec093545e17db0bfb076dd9f37",
        ),
        (
            "structural.pdf",
            include_bytes!("fixtures/structural.pdf"),
            "7f57e0bb0d6777c8d9f018e9627f18de8064fc0d717914c6f03b200e2d2b2b50",
        ),
        (
            "typst-pdfa-1b.pdf",
            include_bytes!("fixtures/typst-pdfa-1b.pdf"),
            "94c23eff7c1b2a16f4f284d662456b4aef56358c9ff225e5f0bac632801ce035",
        ),
    ];

    for (name, bytes, expected) in fixtures {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected, "{name} changed byte-for-byte");
    }
}
