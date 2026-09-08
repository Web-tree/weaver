use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn wvr_is_the_canonical_cli() {
    Command::cargo_bin("wvr")
        .expect("wvr binary should build")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Usage: wvr"))
        .stdout(contains("Declarative directory configuration"));
}
