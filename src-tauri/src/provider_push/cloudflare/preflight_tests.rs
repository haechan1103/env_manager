use super::*;

#[test]
fn parses_only_safe_cloudflare_whoami_fields() {
    let whoami = parse_whoami(
        br#"{
          "loggedIn": true,
          "authType": "OAuth Token",
          "email": "not-returned-to-ui@example.com",
          "accounts": [{"id":"account-1","name":"Team"}],
          "tokenPermissions": ["workers:write"]
        }"#,
        &[],
    )
    .expect("whoami");

    assert!(whoami.logged_in);
    assert_eq!(whoami.auth_type.as_deref(), Some("OAuth Token"));
    assert_eq!(whoami.accounts.expect("accounts")[0].id, "account-1");
}

#[test]
fn parses_unauthenticated_cloudflare_json_from_stderr() {
    let whoami = parse_whoami(&[], br#"{"loggedIn":false}"#).expect("whoami");
    assert!(!whoami.logged_in);
}

#[test]
fn classifies_configured_and_ambiguous_accounts() {
    let accounts = vec![
        CloudflareAccount {
            id: "one".to_owned(),
            name: "One".to_owned(),
        },
        CloudflareAccount {
            id: "two".to_owned(),
            name: "Two".to_owned(),
        },
    ];
    assert_eq!(
        classify_account(Some("one"), &accounts),
        CloudflareAccountState::Matched
    );
    assert_eq!(
        classify_account(Some("missing"), &accounts),
        CloudflareAccountState::Mismatch
    );
    assert_eq!(
        classify_account(None, &accounts),
        CloudflareAccountState::Ambiguous
    );
}
