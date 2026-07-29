use sha2::{Digest, Sha256};

#[test]
fn pdf_fixtures_remain_byte_exact() {
    let fixtures: [(&str, &[u8], &str); 8] = [
        (
            "encrypted.pdf",
            include_bytes!("fixtures/encrypted.pdf"),
            "59f52bf64091ea195ad0315510a724911ae74daa7a916c3c2d68d96a4150e819",
        ),
        (
            "example-1.pdf",
            include_bytes!("fixtures/example-1.pdf"),
            "cdfc6f02b1d22e5074e8ddc42b2c4262015ae83a4ff5ded2c334090f02b4b355",
        ),
        (
            "malformed.pdf",
            include_bytes!("fixtures/malformed.pdf"),
            "df8088e6de6e266de2bcf72a2d1c9e8eee38b393c651184826be82da2877b65d",
        ),
        (
            "not-compliant-1.pdf",
            include_bytes!("fixtures/not-compliant-1.pdf"),
            "d9d256bd545284685108fb3d1977b67bb76369d88b057576cb0cffe462bf1ce0",
        ),
        (
            "poster.pdf",
            include_bytes!("fixtures/poster.pdf"),
            "f9a5f76d8aed003d991a3b22db94c4ea568a27381f1e9214d4b4cd12285d8109",
        ),
        (
            "proposal.pdf",
            include_bytes!("fixtures/proposal.pdf"),
            "293407d3ae3db2fd20b25e7e07050c7d7f7b57d9d097b70cfcd69f251d784f39",
        ),
        (
            "resume.pdf",
            include_bytes!("fixtures/resume.pdf"),
            "2db88d2710d1b424571feb6679c21985079e7843717f7a6513f2c718d69a3a0f",
        ),
        (
            "structural.pdf",
            include_bytes!("fixtures/structural.pdf"),
            "7f57e0bb0d6777c8d9f018e9627f18de8064fc0d717914c6f03b200e2d2b2b50",
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
