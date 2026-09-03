use super::super::integration_requires_repair;

#[test]
fn an_outdated_bundle_is_an_update_not_a_repair() {
    assert!(!integration_requires_repair(true, true, false));
    assert!(integration_requires_repair(true, false, false));
    assert!(!integration_requires_repair(true, false, true));
    assert!(!integration_requires_repair(false, false, false));
}
