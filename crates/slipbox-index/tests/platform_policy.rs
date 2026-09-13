//! Platform capability contracts at the index crate boundary.

use std::fs;

use anyhow::Result;
use slipbox_index::{
    DiscoveryPolicy, ExternalProgram, PlatformPolicy, UnsupportedCapability, read_source,
    read_source_with_platform, scan_path, scan_path_with_platform, scan_path_with_policy,
    scan_root, scan_root_with_platform, scan_root_with_policy,
};
use tempfile::TempDir;

#[test]
fn read_source_reads_a_plain_file() -> Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("plain.org");
    fs::write(&path, "#+title: Plain\n")?;

    assert_eq!(read_source(&path)?, "#+title: Plain\n");
    assert_eq!(
        read_source_with_platform(&path, &PlatformPolicy::headless())?,
        "#+title: Plain\n"
    );
    Ok(())
}

#[test]
fn read_source_refuses_an_encrypted_envelope() -> Result<()> {
    let dir = TempDir::new()?;
    for (name, program) in [
        ("secret.org.gpg", ExternalProgram::Gpg),
        ("secret.org.age", ExternalProgram::Age),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, "envelope bytes")?;

        for error in [
            read_source(&path).expect_err("the convenience read is headless"),
            read_source_with_platform(&path, &PlatformPolicy::headless())
                .expect_err("a headless read of an envelope is refused"),
        ] {
            let refusal = error
                .downcast_ref::<UnsupportedCapability>()
                .expect("the refusal is the typed platform-policy error");
            assert_eq!(refusal.program(), program);
            assert_eq!(refusal.path(), path);
        }
    }
    Ok(())
}

#[test]
fn the_default_platform_policy_refuses_a_requested_envelope() -> Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("secret.org.gpg");
    fs::write(&path, "envelope bytes")?;

    let error = read_source_with_platform(&path, &PlatformPolicy::default())
        .expect_err("the default policy authorizes no external program");

    assert!(
        error
            .downcast_ref::<UnsupportedCapability>()
            .is_some_and(|refusal| refusal.program() == ExternalProgram::Gpg)
    );
    Ok(())
}

#[test]
fn scanning_one_file_refuses_an_envelope_before_running_a_decryptor() -> Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("secret.org.gpg");
    fs::write(&path, "envelope bytes")?;

    let discovery = DiscoveryPolicy::default();
    assert!(
        discovery.matches_path(dir.path(), &path),
        "discovery classifies an envelope as Org, so the platform policy is the only guard"
    );

    for error in [
        scan_path(dir.path(), &path).expect_err("scan_path is headless"),
        scan_path_with_policy(dir.path(), &path, &discovery)
            .expect_err("scan_path_with_policy is headless"),
        scan_path_with_platform(dir.path(), &path, &discovery, &PlatformPolicy::headless())
            .expect_err("an explicit headless scan refuses decryptors"),
    ] {
        assert!(
            error.downcast_ref::<UnsupportedCapability>().is_some(),
            "the scan is refused by platform policy: {error:#}"
        );
    }
    Ok(())
}

#[test]
fn scanning_a_root_refuses_an_envelope_instead_of_decrypting_it() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("plain.org"), "#+title: Plain\n")?;
    fs::write(dir.path().join("secret.org.gpg"), "envelope bytes")?;

    for error in [
        scan_root(dir.path()).expect_err("scan_root is headless"),
        scan_root_with_policy(dir.path(), &DiscoveryPolicy::default())
            .expect_err("scan_root_with_policy is headless"),
        scan_root_with_platform(
            dir.path(),
            &DiscoveryPolicy::default(),
            &PlatformPolicy::headless(),
        )
        .expect_err("an explicit headless root scan refuses decryptors"),
    ] {
        assert!(
            error.downcast_ref::<UnsupportedCapability>().is_some(),
            "the scan is refused by platform policy: {error:#}"
        );
    }
    Ok(())
}

#[test]
fn scanning_a_plain_root_is_unaffected_by_the_headless_policy() -> Result<()> {
    let dir = TempDir::new()?;
    fs::write(dir.path().join("plain.org"), "#+title: Plain\n")?;

    let files = scan_root(dir.path())?;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_path, "plain.org");
    Ok(())
}

#[cfg(feature = "desktop-decryptors")]
#[test]
fn a_desktop_build_still_authorizes_the_decryptors() {
    use std::path::Path;

    let policy = PlatformPolicy::desktop();

    assert_eq!(
        policy
            .authorize_source(Path::new("/notes/secret.org.gpg"))
            .expect("desktop authorizes gpg"),
        Some(ExternalProgram::Gpg)
    );
    assert_eq!(
        policy
            .authorize_source(Path::new("/notes/secret.org.age"))
            .expect("desktop authorizes age"),
        Some(ExternalProgram::Age)
    );
}
