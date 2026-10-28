use lemonfiber_adapters::Disk;
use lemonfiber_fixtures::scratch::Scratch;

use super::super::{kept, kept_or_made, read};
use super::{promoted, replacing, staged, withdrawn, NEXT, PREVIOUS};

#[test]
fn a_replacement_is_handed_beside_the_one_in_force_then_served_then_the_old_withdrawn() {
    let config = Scratch::new("internal-replacing");
    let Ok(old) = kept_or_made(&Disk, &config, "jellyfin") else {
        unreachable!("a certificate is kept");
    };
    assert_eq!(
        replacing(&Disk, &config).map(|kept| (kept.pins(), kept.under_way())),
        Ok((vec![old.fingerprint.clone()], false))
    );

    let Ok(new) = staged(&Disk, &config, "jellyfin") else {
        unreachable!("a replacement is staged");
    };
    assert_eq!(staged(&Disk, &config, "jellyfin").as_ref(), Ok(&new));
    let mut both = vec![old.fingerprint.clone(), new.fingerprint.clone()];
    both.sort();
    let standing = replacing(&Disk, &config);
    assert_eq!(
        standing
            .as_ref()
            .map(|kept| (kept.pins(), kept.under_way())),
        Ok((both.clone(), true))
    );
    assert_eq!(kept(&Disk, &config).ok().flatten(), Some(old.clone()));

    assert_eq!(promoted(&Disk, &config).as_ref(), Ok(&Some(new.clone())));
    assert_eq!(
        promoted(&Disk, &config),
        Ok(None),
        "nothing is left to promote"
    );
    let standing = replacing(&Disk, &config);
    assert_eq!(
        standing.as_ref().map(|kept| (
            kept.current.clone(),
            kept.next.clone(),
            kept.previous.clone(),
            kept.pins()
        )),
        Ok((Some(new.clone()), None, Some(old), both))
    );

    assert_eq!(withdrawn(&Disk, &config), Ok(()));
    assert_eq!(
        replacing(&Disk, &config).map(|kept| (kept.pins(), kept.under_way())),
        Ok((vec![new.fingerprint], false))
    );
}

#[test]
fn a_promotion_stopped_part_way_is_taken_up_without_keeping_the_new_one_aside() {
    let config = Scratch::new("internal-resumed");
    let Ok(new) = staged(&Disk, &config, "jellyfin") else {
        unreachable!("a replacement is staged");
    };
    assert_eq!(promoted(&Disk, &config).as_ref(), Ok(&Some(new.clone())));
    let standing = replacing(&Disk, &config);
    assert_eq!(
        standing
            .as_ref()
            .map(|kept| (kept.current.clone(), kept.previous.clone())),
        Ok((Some(new.clone()), None)),
        "with nothing in force, nothing is kept aside"
    );

    let _ = std::fs::write(config.join(NEXT), new.pkcs12());
    assert_eq!(promoted(&Disk, &config).as_ref(), Ok(&Some(new.clone())));
    assert!(!config.join(PREVIOUS).exists());
    assert!(!config.join(NEXT).exists());
}

#[test]
fn a_certificate_says_which_names_it_is_made_out_to() {
    let Ok(made) = super::super::made_for("jellyfin") else {
        unreachable!("a certificate is made");
    };
    assert!(made.made_out_to("jellyfin"));
    assert!(!made.made_out_to("emby"));
    let reread = read(made.pkcs12());
    assert!(reread.is_ok_and(|kept| kept.made_out_to("jellyfin") && !kept.made_out_to("emby")));

    let Ok(addressed) = super::super::made_for("127.0.0.1") else {
        unreachable!("a certificate is made");
    };
    assert!(read(addressed.pkcs12()).is_ok_and(|kept| !kept.made_out_to("127.0.0.1")));
}

#[test]
fn what_cannot_be_read_or_written_is_said() {
    let config = Scratch::new("internal-replacing-unread");
    let _ = std::fs::create_dir_all(config.join("lemonfiber"));
    let _ = std::fs::write(config.join(NEXT), "not a certificate");
    assert!(replacing(&Disk, &config).is_err());
    assert!(staged(&Disk, &config, "jellyfin").is_err());
    assert!(promoted(&Disk, &config).is_err());

    let _ = std::fs::remove_file(config.join(NEXT));
    let _ = std::fs::write(config.join(super::super::INTERNAL), "not a certificate");
    assert!(staged(&Disk, &config, "jellyfin").is_ok());
    assert!(promoted(&Disk, &config).is_err());
    let _ = std::fs::create_dir_all(config.join(PREVIOUS));
    assert!(withdrawn(&Disk, &config).is_err());
}
