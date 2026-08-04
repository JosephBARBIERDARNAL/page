use std::fs;

use sha2::{Digest, Sha256};

#[test]
fn pdf_fixtures_remain_byte_exact() {
    let fixtures: [(&str, &[u8], &str); 25] = [
        (
            "canonical-pdfa-1a.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a.pdf"),
            "efa925f02cdb02eb127a22d09810f2d564ba998f6a45909bcee8e07fb841e96b",
        ),
        (
            "canonical-pdfa-1b.pdf",
            include_bytes!("fixtures/canonical-pdfa-1b.pdf"),
            "27e17de2b43a963ccc385dddd223d377d629b3ee882aac14b6aa633483efbc88",
        ),
        (
            "canonical-pdfa-1a-unused-invalid-font.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-unused-invalid-font.pdf"),
            "2f354d3d7a5513ba82de3635c6f8862a5a6602dddf2701f6abd64c2462114c28",
        ),
        (
            "canonical-pdfa-1a-fonts.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-fonts.pdf"),
            "b1c943188feb5001dc65d152321722fc2bc77f94d7e8f6ccf7f525b72d535052",
        ),
        (
            "canonical-pdfa-1a-structure.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-structure.pdf"),
            "54365af93f025b44229af869543aa28d662c9d8181ae0dabfd3e3098e13a7c2f",
        ),
        (
            "canonical-pdfa-1a-content.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-content.pdf"),
            "5910a8bf636db7f6f11d68926f4f084548bed4adbada8d23a2885a4f6a4a6937",
        ),
        (
            "canonical-pdfa-1a-annotations.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-annotations.pdf"),
            "7c73ecb9fee423a7bd7538a79deff5b473ad5000acbb3ae4f02bd56d4a048b40",
        ),
        (
            "canonical-pdfa-1a-forms.pdf",
            include_bytes!("fixtures/canonical-pdfa-1a-forms.pdf"),
            "803aca6a6132c80a6a03209229bc7c11c44d6c8d40871b856451f981c88f8a49",
        ),
        (
            "mutations/PDFA1A-ID-CONFORMANCE-001/id_conformance_b.pdf",
            include_bytes!("fixtures/mutations/PDFA1A-ID-CONFORMANCE-001/id_conformance_b.pdf"),
            "4535f3418740ce757c62bcdbe20d1b563f0783f25445c2aa8a49d3c8998d1074",
        ),
        (
            "mutations/PDFA1A-TAGGED-DOCUMENT-001/tagged_missing.pdf",
            include_bytes!("fixtures/mutations/PDFA1A-TAGGED-DOCUMENT-001/tagged_missing.pdf"),
            "2544eb85653838af6fc33cd1fb4e28edf38923f876480ce577ce11189f8be0c8",
        ),
        (
            "mutations/PDFA1A-STRUCT-TREE-ROOT-001/struct_tree_missing.pdf",
            include_bytes!(
                "fixtures/mutations/PDFA1A-STRUCT-TREE-ROOT-001/struct_tree_missing.pdf"
            ),
            "8cc643b739c5da39316cdfa5e26da4197189a62ad169fdd7cb175150b56955f4",
        ),
        (
            "mutations/PDFA1A-STRUCT-TREE-ROLE-MAP-001/role_map_wrong_type.pdf",
            include_bytes!(
                "fixtures/mutations/PDFA1A-STRUCT-TREE-ROLE-MAP-001/role_map_wrong_type.pdf"
            ),
            "ffdf3c95104df1498ce492dbe5083b4867ebe2d8b3d886ba5217dd545f5e6fc9",
        ),
        (
            "mutations/PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001/role_map_cycle.pdf",
            include_bytes!(
                "fixtures/mutations/PDFA1A-STRUCT-TREE-ROLE-MAP-CYCLE-001/role_map_cycle.pdf"
            ),
            "f03f48572a611c42b890577e3864965fee27b9f95c16d5ed3bffd343ef61d8d1",
        ),
        (
            "mutations/PDFA1A-LANG-001/language_missing.pdf",
            include_bytes!("fixtures/mutations/PDFA1A-LANG-001/language_missing.pdf"),
            "c8e6653e277983b5280c9b2bab5b9575d2c8d62bc0a8b782d859bc2a18e27149",
        ),
        (
            "mutations/PDFA1A-UNICODE-MAPPING-001/unicode_missing.pdf",
            include_bytes!("fixtures/mutations/PDFA1A-UNICODE-MAPPING-001/unicode_missing.pdf"),
            "3e12fd8f734be9745476077f024fd37db1117740145a9cba1f80c1f877c53026",
        ),
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
            "715a7f0da1f41a90a4905e1b138ba08673fa415a90cdeede9929e7665077a7e7",
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
            "82b98675997387850f6bf54d05c748893a6ef44abd92510d92665379f66e00db",
        ),
        (
            "typst-pdfa-1a.pdf",
            include_bytes!("fixtures/typst-pdfa-1a.pdf"),
            "9af2921f745270671948f053f62b7a0e688fcccf8b465293d57c7b572c44d7d2",
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

#[test]
fn shared_mutation_fixtures_remain_byte_exact() {
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read("tests/fixtures/verapdf-diff-cases.json")
            .expect("read shared differential manifest"),
    )
    .expect("parse shared differential manifest");
    for mutation in manifest["checked_in_mutations"]
        .as_array()
        .expect("checked-in mutation list")
    {
        let path = mutation["path"].as_str().expect("mutation path");
        let expected = mutation["sha256"].as_str().expect("mutation hash");
        let actual = Sha256::digest(
            fs::read(path)
                .unwrap_or_else(|error| panic!("read checked-in mutation {path}: {error}")),
        )
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
        assert_eq!(actual, expected, "{path} changed byte-for-byte");
    }
}
