use super::error::*;

#[test]
fn error_display() {
    let err = SakiError::ConnectionFailed("host unreachable".into());
    assert_eq!(err.to_string(), "Connection failed: host unreachable");
}

#[test]
fn error_serializes() {
    let err = SakiError::AuthFailed;
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("AuthFailed"));
}
