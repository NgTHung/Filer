use filer_task_web::device_label::{FALLBACK_LABEL, RECOVERY_CLI_LABEL, from_user_agent};

#[test]
fn recognized_browsers_yield_name_and_major_version() {
    let chrome = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    assert_eq!(from_user_agent(Some(chrome)), "Chrome 131");

    let firefox = "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0";
    assert_eq!(from_user_agent(Some(firefox)), "Firefox 131");

    let curl = "curl/8.7.1";
    assert_eq!(from_user_agent(Some(curl)), "curl 8");
}

#[test]
fn edge_is_not_mistaken_for_chrome() {
    let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.2903.63";
    assert_eq!(from_user_agent(Some(edge)), "Edge 131");
}

#[test]
fn safari_version_comes_from_version_token_not_the_engine() {
    let safari = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
    assert_eq!(from_user_agent(Some(safari)), "Safari 17");
}

#[test]
fn missing_and_unparsable_user_agents_fall_back_to_a_stable_placeholder() {
    assert_eq!(from_user_agent(None), FALLBACK_LABEL);
    assert_eq!(from_user_agent(Some("")), FALLBACK_LABEL);
    assert_eq!(from_user_agent(Some("   ")), FALLBACK_LABEL);
    assert_eq!(
        from_user_agent(Some("totally not a browser")),
        FALLBACK_LABEL
    );
}

#[test]
fn known_browser_tokens_without_a_major_version_fall_back() {
    assert_eq!(
        from_user_agent(Some("Mozilla/5.0 Chrome/not-a-version")),
        FALLBACK_LABEL
    );
    assert_eq!(
        from_user_agent(Some("Mozilla/5.0 Safari/605.1.15")),
        FALLBACK_LABEL
    );
}

#[test]
fn recovery_cli_label_is_a_distinct_stable_placeholder() {
    assert_eq!(RECOVERY_CLI_LABEL, "Recovery CLI");
    assert_ne!(RECOVERY_CLI_LABEL, FALLBACK_LABEL);
}
