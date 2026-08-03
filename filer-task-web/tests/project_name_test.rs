use filer_task_web::project_name::validated;

#[test]
fn an_ordinary_directory_name_is_accepted_and_trimmed() {
    assert_eq!(validated("new-thing").ok(), Some("new-thing"));
    assert_eq!(validated("  new-thing  ").ok(), Some("new-thing"));
    assert_eq!(validated("Filer.Docs").ok(), Some("Filer.Docs"));
}

#[test]
fn a_name_that_is_only_whitespace_is_refused() {
    assert!(validated("").is_err());
    assert!(validated("   ").is_err());
}

#[test]
fn a_name_that_reaches_out_of_the_chosen_directory_is_refused() {
    for name in [
        "nested/deep",
        "nested\\deep",
        "/absolute",
        ".",
        "..",
        "../elsewhere",
    ] {
        assert!(validated(name).is_err(), "{name:?} was accepted");
    }
}
