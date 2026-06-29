use crate::vfs::secret::{ProviderSecret, REDACTED_SECRET};

#[test]
fn provider_secret_debug_redacts_inner_value() {
    let secret = ProviderSecret::new("actual-password");
    let debug = format!("{secret:?}");

    assert_eq!(secret.as_str(), "actual-password");
    assert!(debug.contains(REDACTED_SECRET));
    assert!(!debug.contains("actual-password"));
}

#[derive(Debug)]
#[allow(dead_code)]
struct RuntimeProviderConfig {
    password: ProviderSecret,
    secret_key: ProviderSecret,
    session_token: ProviderSecret,
    bearer_token: ProviderSecret,
    private_key: ProviderSecret,
}

#[test]
fn runtime_provider_config_debug_redacts_secret_fields() {
    let config = RuntimeProviderConfig {
        password: ProviderSecret::new("password-value"),
        secret_key: ProviderSecret::new("secret-key-value"),
        session_token: ProviderSecret::new("session-token-value"),
        bearer_token: ProviderSecret::new("bearer-token-value"),
        private_key: ProviderSecret::new("private-key-value"),
    };

    let debug = format!("{config:?}");

    assert!(debug.contains("password"));
    assert!(debug.contains("secret_key"));
    assert!(debug.contains("session_token"));
    assert!(debug.contains("bearer_token"));
    assert!(debug.contains("private_key"));
    assert!(debug.contains(REDACTED_SECRET));
    assert!(!debug.contains("password-value"));
    assert!(!debug.contains("secret-key-value"));
    assert!(!debug.contains("session-token-value"));
    assert!(!debug.contains("bearer-token-value"));
    assert!(!debug.contains("private-key-value"));
}
