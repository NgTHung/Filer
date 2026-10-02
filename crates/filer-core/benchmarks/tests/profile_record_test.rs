use filer_core_benchmarks::{ErrorCode, ProfileRecord};
use sha2::{Digest, Sha256};

fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

fn token(hasher: &mut Sha256, value: &str) {
    hasher.update(format!("{}:{value}", value.len()).as_bytes());
}

fn independent_profile_digest(pairs: &[(&str, &str)]) -> String {
    let mut hasher = Sha256::new();
    for value in ["filer-benchmark-digest-v1", "profile", "2", "name", "value"] {
        token(&mut hasher, value);
    }
    token(&mut hasher, &pairs.len().to_string());
    for (name, value) in pairs {
        token(&mut hasher, name);
        token(&mut hasher, value);
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[test]
fn digests_records_in_their_declared_order() {
    let pairs = [
        ("os", "linux"),
        ("kernel", "6.12.107"),
        ("logical_cpus", "16"),
    ];
    let record =
        ProfileRecord::new("linux-x86_64-lab-01", entries(&pairs)).expect("record is valid");

    assert_eq!(record.id(), "linux-x86_64-lab-01");
    assert_eq!(record.entries(), entries(&pairs).as_slice());
    assert_eq!(record.digest(), independent_profile_digest(&pairs));

    let reordered = ProfileRecord::new(
        "linux-x86_64-lab-01",
        entries(&[
            ("kernel", "6.12.107"),
            ("os", "linux"),
            ("logical_cpus", "16"),
        ]),
    )
    .expect("record is valid");
    assert_ne!(reordered.digest(), record.digest());
}

#[test]
fn rejects_invalid_profile_ids_and_names() {
    for (id, pairs) in [
        ("-starts-with-dash", vec![("os", "linux")]),
        ("lab-01", vec![("", "linux")]),
        ("lab-01", vec![("os", "linux"), ("os", "windows")]),
    ] {
        let error = ProfileRecord::new(id, entries(&pairs)).expect_err("record is invalid");
        assert_eq!(error.code(), ErrorCode::InvalidSchema);
    }
}
