use super::super::model::is_update_available;

#[test]
fn update_detection_uses_semantic_precedence_without_downgrading() {
    assert!(is_update_available("0.5.0", "1.0.0"));
    assert!(!is_update_available("1.0.0", "1.0.0"));
    assert!(!is_update_available("1.1.0", "1.0.0"));
    assert!(!is_update_available("1.0.0+codex.local", "1.0.0"));
}
