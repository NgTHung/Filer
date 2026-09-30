use std::fs;
use std::path::PathBuf;

use filer_core_benchmarks::{Field, Kind, ManifestErrorCode, ValidatedManifest, canonical_digest};

fn manifest(name: &str) -> ValidatedManifest {
    let file_name = if name.ends_with(".json") {
        name.to_string()
    } else {
        format!("{name}.json")
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join(file_name);
    ValidatedManifest::load(path).expect("normative manifest should validate")
}

#[test]
fn both_normative_manifests_validate_against_literal_digests() {
    let flat_10k = manifest("flat-10k-v1.json");
    let flat_100k = manifest("flat-100k-v1.json");

    assert_eq!(flat_10k.id(), "flat-10k-v1");
    assert_eq!(flat_10k.entry_count(), 10_000);
    assert_eq!(
        flat_10k.manifest_digest(),
        "sha256:b684d98507db303ffc02805732bfc721d65af093db142b32887c35b0d0e5a95e"
    );
    assert_eq!(
        flat_10k.membership_digest(),
        "sha256:ac84c83acce8b289874fc468a4aa77b113404ff041e54af0ba588cc1e2c905a9"
    );
    assert_eq!(
        flat_10k.metadata_digest(),
        "sha256:581febb0f3f93343ada6474ed2d99d706c0a03494c303267968e158454a2ef81"
    );
    assert_eq!(
        flat_10k.name_order_digest(),
        "sha256:a83cb70360ea0d060fde788c3a5c0ab30fa2be56c0c4ac2bb604f78b8aa8ac22"
    );
    assert_eq!(flat_10k.expected().filter_count, Some(90));
    assert_eq!(
        flat_10k.name_viewport_digest(),
        "sha256:f8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a"
    );
    assert_eq!(
        flat_10k.filter_order_digest(),
        Some("sha256:193ef6adbff68a4b7fad2050ddb8b85e3c830484bbe3bcc4e557b86cb43161ea")
    );
    assert_eq!(
        flat_10k.filter_viewport_digest(),
        Some("sha256:76d10051562590383e451a7965ff09b6af26a0e832ac329fa478aacc3663fc92")
    );

    assert_eq!(flat_100k.id(), "flat-100k-v1");
    assert_eq!(flat_100k.entry_count(), 100_000);
    assert_eq!(
        flat_100k.manifest_digest(),
        "sha256:dd55706f9be421dafc15a7237493158c3703ea9a0e379e7d51f6653020e1566e"
    );
    assert_eq!(
        flat_100k.membership_digest(),
        "sha256:b32e48f7e2c31b06e7c5256e624b61c7f3d3bcbc4ede922a145ef7a4d544f1d3"
    );
    assert_eq!(
        flat_100k.metadata_digest(),
        "sha256:d2dd00efed47da4987dc30ae8bd558cfad45d9aa9ce34d2b0f2f5ccb2349831d"
    );
    assert_eq!(
        flat_100k.name_viewport_digest(),
        "sha256:f8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a"
    );
    assert_eq!(flat_100k.expected().filter_count, None);
}

#[test]
fn canonical_digest_uses_length_prefixed_tokens_and_null_sizes() {
    let rows = vec![
        filer_core_benchmarks::CanonicalRow::new("b", Kind::Directory, None, -2),
        filer_core_benchmarks::CanonicalRow::new("a", Kind::File, Some(7), 1),
    ];
    let digest = canonical_digest("membership", &[Field::Identity], &rows);

    assert_eq!(
        digest,
        "sha256:61eb781ae396d8acce8a74f494175c0e0c65ae5b98d7bdb13da60006571e972b"
    );
    assert_eq!(rows[0].value(Field::SizeBytes), "~");
    assert_eq!(rows[0].value(Field::ModifiedUnixNs), "-2");
}

#[test]
fn documented_one_row_page_digest_is_stable() {
    let row = filer_core_benchmarks::CanonicalRow::new(
        ".dir-000000",
        Kind::Directory,
        None,
        1_704_067_200_000_000_000,
    );

    assert_eq!(
        canonical_digest("page", &[Field::Identity, Field::Kind], &[row]),
        "sha256:b01ec7b38aa3ead7298b439da888bd9e943cb15bd431d05d3dee13698bb4c2f9"
    );
}

#[test]
fn generated_rows_cover_hidden_entries_directories_extensions_sizes_and_timestamps() {
    let rows = manifest("flat-10k-v1").expected_rows();
    let samples = [
        (
            0,
            ".dir-000000",
            Kind::Directory,
            None,
            1_704_067_200_000_000_000,
        ),
        (
            1,
            "file-000001.txt",
            Kind::File,
            Some(7920),
            1_704_067_201_000_000_000,
        ),
        (
            10,
            "dir-000010",
            Kind::Directory,
            None,
            1_704_067_210_000_000_000,
        ),
        (
            25,
            ".file-000025.txt",
            Kind::File,
            Some(197_976),
            1_704_067_225_000_000_000,
        ),
    ];

    for (index, identity, kind, size, modified) in samples {
        let row = &rows[index];
        assert_eq!(row.identity, identity);
        assert_eq!(row.kind, kind);
        assert_eq!(row.size_bytes, size);
        assert_eq!(row.modified_unix_ns, modified);
    }
}

#[test]
fn membership_and_metadata_are_order_independent_but_named_output_is_not() {
    let manifest = manifest("flat-10k-v1");
    let rows = manifest.expected_rows();
    let mut reordered = rows[..128].to_vec();
    reordered.reverse();

    assert_eq!(
        manifest.digest_rows("membership", &[Field::Identity], &reordered),
        canonical_digest("membership", &[Field::Identity], &reordered)
    );

    let ordered = manifest.digest_rows("ordered", &[Field::Identity, Field::Kind], &rows[..128]);
    let reversed = manifest.digest_rows("ordered", &[Field::Identity, Field::Kind], &reordered);
    assert_ne!(ordered, reversed);
}

#[test]
fn both_complete_row_sets_match_the_normative_expected_values() {
    let flat_10k = manifest("flat-10k-v1");
    let flat_100k = manifest("flat-100k-v1");
    let metadata_fields = [
        Field::Identity,
        Field::Kind,
        Field::SizeBytes,
        Field::ModifiedUnixNs,
    ];

    assert_eq!(
        canonical_digest("membership", &[Field::Identity], &flat_10k.expected_rows()),
        flat_10k.membership_digest()
    );
    assert_eq!(
        canonical_digest("metadata", &metadata_fields, &flat_10k.expected_rows()),
        flat_10k.metadata_digest()
    );
    assert_eq!(
        canonical_digest("membership", &[Field::Identity], &flat_100k.expected_rows()),
        flat_100k.membership_digest()
    );
    assert_eq!(
        canonical_digest("metadata", &metadata_fields, &flat_100k.expected_rows()),
        flat_100k.metadata_digest()
    );
    assert_eq!(
        flat_10k.expected_filter_rows().as_ref().map(Vec::len),
        Some(90)
    );
    assert_eq!(flat_10k.expected_name_rows()[..40].len(), 40);
    assert_eq!(flat_100k.expected_name_rows()[..40].len(), 40);
}

#[test]
fn altered_manifest_expectations_and_parameters_are_rejected() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("manifests")
        .join("flat-10k-v1.json");
    let source = fs::read_to_string(path).expect("manifest should be readable");

    for (from, to, expected_code) in [
        (
            "ac84c83acce8b289874fc468a4aa77b113404ff041e54af0ba588cc1e2c905a9",
            "bc84c83acce8b289874fc468a4aa77b113404ff041e54af0ba588cc1e2c905a9",
            ManifestErrorCode::ExpectedMismatch,
        ),
        ("7919", "7920", ManifestErrorCode::ExpectedMismatch),
        (
            "\"filter_count\":90",
            "\"filter_count\":91",
            ManifestErrorCode::ExpectedMismatch,
        ),
        (
            "sha256:f8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a",
            "sha256:e8637ffa842a13652e47c6523d87838eef4849433896fd37c9e746b42163ce3a",
            ManifestErrorCode::ExpectedMismatch,
        ),
        (
            "b684d98507db303ffc02805732bfc721d65af093db142b32887c35b0d0e5a95e",
            "c684d98507db303ffc02805732bfc721d65af093db142b32887c35b0d0e5a95e",
            ManifestErrorCode::DigestMismatch,
        ),
    ] {
        let mutated = source.replacen(from, to, 1);
        let error =
            ValidatedManifest::from_bytes(mutated.as_bytes()).expect_err("mutation must fail");
        assert_eq!(error.code(), expected_code, "mutation {from}");
    }
}
