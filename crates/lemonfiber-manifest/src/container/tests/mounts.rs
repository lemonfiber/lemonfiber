use super::super::Mount;
use super::{plain, wrong, NONE};

#[test]
fn a_mount_comes_from_inside_its_own_directory_and_lands_outside_the_data() {
    let none = NONE;
    let mounts = [
        Mount {
            from: "cache".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
        Mount {
            from: "../etc".to_owned(),
            to: "/etc/x".to_owned(),
            ro: true,
        },
        Mount {
            from: "/etc".to_owned(),
            to: "/data/x".to_owned(),
            ro: false,
        },
        Mount {
            from: "again".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
    ];
    let mut mounting = plain(&none);
    mounting.mounts = &mounts;
    let said = wrong(&mounting);
    assert_eq!(said.len(), 4, "{said:#?}");
    assert!(said
        .iter()
        .any(|line| line.contains("../etc is not a relative path")));
    assert!(said
        .iter()
        .any(|line| line.contains("/data/x is inside the data mount")));
    assert!(said
        .iter()
        .any(|line| line.contains("/cache is the target of another mount")));
}

#[test]
fn no_mount_source_lies_inside_another() {
    let none = NONE;
    let mounts = [
        Mount {
            from: "cache".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
        Mount {
            from: "cache/deep".to_owned(),
            to: "/deep".to_owned(),
            ro: false,
        },
        Mount {
            from: "cachet".to_owned(),
            to: "/cachet".to_owned(),
            ro: false,
        },
    ];
    let mut nested = plain(&none);
    nested.mounts = &mounts;
    let said = wrong(&nested);
    assert_eq!(
        said,
        ["service jellyfin.mount /deep: cache/deep lies inside cache, another of its sources"]
    );
}

#[test]
fn two_mounts_from_one_source_are_each_refused_naming_the_mount() {
    let none = NONE;
    let mounts = [
        Mount {
            from: "cache".to_owned(),
            to: "/cache".to_owned(),
            ro: false,
        },
        Mount {
            from: "cache".to_owned(),
            to: "/again".to_owned(),
            ro: true,
        },
        Mount {
            from: "logs".to_owned(),
            to: "/logs".to_owned(),
            ro: false,
        },
    ];
    let mut doubled = plain(&none);
    doubled.mounts = &mounts;
    assert_eq!(
        wrong(&doubled),
        [
            "service jellyfin.mount /cache: cache is the source of another of its mounts",
            "service jellyfin.mount /again: cache is the source of another of its mounts",
        ]
    );
}
