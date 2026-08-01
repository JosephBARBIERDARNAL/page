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
            "278add4cefbf769806547b661d792df63eda95fcd82e643f791cdeeeaf705a6e",
        ),
        (
            "malformed.pdf",
            include_bytes!("fixtures/malformed.pdf"),
            "df8088e6de6e266de2bcf72a2d1c9e8eee38b393c651184826be82da2877b65d",
        ),
        (
            "not-compliant-1.pdf",
            include_bytes!("fixtures/not-compliant-1.pdf"),
            "c9220b8c7336ae1b8a6f1430b28fe6b55d97a4a29ee0376640b8e3ba9b8d124d",
        ),
        (
            "poster.pdf",
            include_bytes!("fixtures/poster.pdf"),
            "417377a7fa9b2dda8dc2f0a6bda70d228c7f4e026e10e95256a82011a5d5dabd",
        ),
        (
            "proposal.pdf",
            include_bytes!("fixtures/proposal.pdf"),
            "ced967b642b023ab1cfbdad510f496b9be4d09f9c55b52e5e27ddc8e71db5565",
        ),
        (
            "resume.pdf",
            include_bytes!("fixtures/resume.pdf"),
            "98df4b5bb63cc7f3c2f55e67604aad769c43868bce0d8f70b0e6630d2fcb667f",
        ),
        (
            "structural.pdf",
            include_bytes!("fixtures/structural.pdf"),
            "7f57e0bb0d6777c8d9f018e9627f18de8064fc0d717914c6f03b200e2d2b2b50",
        ),
        (
            "typst-pdfa-1b.pdf",
            include_bytes!("fixtures/typst-pdfa-1b.pdf"),
            "e4df432cc9934c3d8e4596a9c0cab4afe4bd524966f3cf654cd642e2adb56b65",
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
