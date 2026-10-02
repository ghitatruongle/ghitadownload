#[test]
fn package_version_matches_release() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.0.3");
}
