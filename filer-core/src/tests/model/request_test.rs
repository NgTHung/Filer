use crate::model::request::RequestId;

#[test]
fn new_request_ids_are_monotonic() {
    let first = RequestId::new();
    let second = RequestId::new();

    assert!(second.0 > first.0);
}

#[test]
fn default_request_id_is_sentinel_zero() {
    assert_eq!(RequestId::default(), RequestId::DEFAULT);
    assert_eq!(RequestId::DEFAULT.0, 0);
    assert_eq!(RequestId::DEFAULT.to_string(), "request:0");
}
