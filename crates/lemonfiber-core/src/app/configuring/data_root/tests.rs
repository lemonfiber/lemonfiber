use std::path::{Path, PathBuf};

use lemonfiber_fixtures::scratch::Scratch;

use super::refusal;
use crate::platform::Environment;
use crate::test_support::a_context;

/// A home nothing on any machine is at, so it resolves as written on every platform.
const HOME: &str = "/Users/nobody-lemonfiber-test";

/// A stack directory of the test's own, on the real disk.
fn machine(name: &str) -> (Scratch, PathBuf) {
    let dir = Scratch::new(name);
    let stack = dir.join("stack");
    let _ = std::fs::create_dir_all(&stack);
    (dir, stack)
}

/// A stack with nothing in it, compiled in, whose directory is wherever the settings say.
static STACKLET: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A context with `home` and the stack in `stack`, on `environment`.
fn ctx(home: Option<&Path>, stack: &Path, environment: Environment) -> crate::app::Ctx {
    a_context()
        .over(crate::stack::Source::Embedded(&STACKLET))
        .environment(environment)
        .settings(crate::config::Settings {
            home: home.map(Path::to_path_buf),
            stack_dir: Some(stack.to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build()
}

/// A Linux context with [`HOME`] and the stack in `stack`.
fn linux(stack: &Path) -> crate::app::Ctx {
    ctx(Some(Path::new(HOME)), stack, Environment::LinuxNative)
}

/// Whether `value` is refused as the data root.
async fn refused(ctx: &crate::app::Ctx, value: &str) -> bool {
    refusal(ctx, value).await.is_some()
}

/// The whole machine and every one of the system's own trees are refused, at the tree
/// and beneath it.
#[tokio::test]
async fn every_system_tree_is_refused() {
    let (_dir, stack) = machine("data-root-system");
    let ctx = linux(&stack);

    for tree in [
        "/",
        "/etc",
        "/etc/media",
        "/usr",
        "/usr/local/media",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64/media",
        "/libx32",
        "/boot",
        "/dev/media",
        "/proc",
        "/sys/media",
        "/var",
        "/var/lib/media",
        "/root",
        "/root/media",
        "/private/media",
        "/System/media",
        "/Applications/media",
        "/Library/media",
        "/run",
        "/run/docker.sock",
        "/var/run/media",
        "/tmp/media",
        "/snap/media",
        "/nix/store",
        "/lost+found",
        "/opt/containerd/media",
        "/run/media",
    ] {
        assert!(refused(&ctx, tree).await, "{tree} is refused");
    }
}

/// The home, anything above it, a hidden directory in it and the platform's own library
/// there are refused; a directory of the operator's own in it is taken.
#[tokio::test]
async fn the_home_is_refused_and_what_is_the_operators_in_it_is_taken() {
    let (_dir, stack) = machine("data-root-home");
    let ctx = linux(&stack);

    for refused_root in [
        HOME.to_owned(),
        "/Users".to_owned(),
        format!("{HOME}/.ssh"),
        format!("{HOME}/.config/media"),
        format!("{HOME}/Library/media"),
        format!("{HOME}/media/.cache/films"),
        format!("{HOME}/Movies/.later"),
    ] {
        assert!(
            refused(&ctx, &refused_root).await,
            "{refused_root} is refused"
        );
    }
    for fine in [format!("{HOME}/media"), format!("{HOME}/Movies/Library")] {
        assert!(!refused(&ctx, &fine).await, "{fine} is taken");
    }
}

/// A pool or a share at the top of the filesystem, and the places systems keep for data
/// and drives, are the operator's to choose.
#[tokio::test]
async fn the_roots_a_nas_keeps_its_data_in_are_taken() {
    let (_dir, stack) = machine("data-root-nas");
    let ctx = linux(&stack);

    for fine in [
        "/tank",
        "/tank/media",
        "/data",
        "/storage/media",
        "/pool/media",
        "/opt/media",
        "/share/media",
        "/srv/media",
        "/mnt/disk/media",
        "/media/drive",
        "/Volumes/Drive/media",
        "/run/media/op/drive",
    ] {
        assert!(!refused(&ctx, fine).await, "{fine} is taken");
    }
}

/// A directory drives or homes are kept beneath is refused as a whole, since it holds
/// every one of them; a directory beneath it is not refused for that.
#[tokio::test]
async fn a_base_is_refused_whole_and_taken_beneath() {
    let (_dir, stack) = machine("data-root-bases");
    let ctx = linux(&stack);

    for base in [
        "/home",
        "/Users",
        "/mnt",
        "/media",
        "/srv",
        "/Volumes",
        "/opt",
        "/run/media",
    ] {
        assert!(refused(&ctx, base).await, "{base} is refused");
    }
    for fine in ["/mnt/tank", "/opt/media", "/srv/media", "/Volumes/Drive"] {
        assert!(!refused(&ctx, fine).await, "{fine} is taken");
    }
}

/// Another user's home is refused, wherever in it; the operator's own is not another's.
#[tokio::test]
async fn another_users_home_is_refused() {
    let (_dir, stack) = machine("data-root-other-home");
    let ctx = linux(&stack);

    for theirs in [
        "/home/someone",
        "/home/someone/media",
        "/Users/Shared/media",
    ] {
        assert!(refused(&ctx, theirs).await, "{theirs} is refused");
    }
    assert!(!refused(&ctx, &format!("{HOME}/media")).await);
}

/// Where the operator's home is not known, or is the filesystem root, nothing outside a
/// system tree is taken either: nothing could rule out that it holds the home.
#[tokio::test]
async fn an_unknown_home_refuses_everything() {
    let stack = Path::new("/tank/lemonfiber-test-stack");
    let unknown = ctx(None, stack, Environment::LinuxNative);
    let rooted = ctx(Some(Path::new("/")), stack, Environment::LinuxNative);

    for value in ["/tank/media", "/mnt/tank", "./data"] {
        for homeless in [&unknown, &rooted] {
            let said = refusal(homeless, value).await.unwrap_or_default();
            assert!(
                said.contains("home directory is not known"),
                "{value}: {said}"
            );
        }
    }
}

/// Inside the stack's directory is taken, written relative to it or not; a relative path
/// that leaves it, or one written with `..`, is refused.
#[tokio::test]
async fn inside_the_stack_is_taken_and_nothing_climbs_out() {
    let stack = Path::new("/tank/lemonfiber-test-stack");
    let ctx = linux(stack);
    let inside = stack.join("media").display().to_string();

    for fine in ["./data", "data", inside.as_str()] {
        assert!(!refused(&ctx, fine).await, "{fine} is taken");
    }
    for refused_root in [".", "..", "./data/../..", "/tank/../etc", "./.hidden/../x"] {
        assert!(
            refused(&ctx, refused_root).await,
            "{refused_root} is refused"
        );
    }
}

/// A relative path inside a stack that itself lies in a system tree is judged by where it
/// lands, as any other path is.
#[tokio::test]
async fn a_relative_path_is_judged_where_it_lands() {
    let ctx = linux(Path::new("/var/lib/lemonfiber-test-stack"));

    assert!(refused(&ctx, "./data").await);
}

/// A link on the way is judged by where it leads: one into a system tree, or one in the
/// stack's directory leading out of it, is refused.
#[cfg(unix)]
#[tokio::test]
async fn a_link_is_judged_by_where_it_leads() {
    let (dir, stack) = machine("data-root-linked");
    let ctx = linux(&stack);
    let into_etc = dir.join("into-etc");
    let into_var = dir.join("into-var");
    let _ = std::os::unix::fs::symlink("/etc", &into_etc);
    let _ = std::os::unix::fs::symlink("/var", &into_var);
    let _ = std::os::unix::fs::symlink("/etc", stack.join("data"));

    for linked in [
        into_etc.display().to_string(),
        into_etc.join("media").display().to_string(),
        into_var.join("media").display().to_string(),
    ] {
        assert!(refused(&ctx, &linked).await, "{linked} is refused");
    }
    assert!(refused(&ctx, "./data").await);
}

/// A link that leads somewhere not there yet cannot be judged by where it leads, so it is
/// refused, wherever it sits.
#[cfg(unix)]
#[tokio::test]
async fn a_link_leading_nowhere_yet_is_refused() {
    let (dir, stack) = machine("data-root-dangling");
    let ctx = linux(&stack);
    let dangling = dir.join("dangling");
    let _ = std::os::unix::fs::symlink("/etc/lemonfiber-nowhere-yet", &dangling);

    let said = refusal(&ctx, &dangling.join("media").display().to_string())
        .await
        .unwrap_or_default();

    assert!(
        said.contains("leads somewhere that is not there yet"),
        "{said}"
    );
}

/// Where the filesystem ignores case, so does the judgement: `/ETC` is `/etc`, and the
/// home spelled in other letters is still the home. Where it does not, they are other
/// directories.
#[tokio::test]
async fn case_is_ignored_where_the_filesystem_ignores_it() {
    let (_dir, stack) = machine("data-root-case");
    let mac = ctx(Some(Path::new(HOME)), &stack, Environment::MacOs);
    let linux = linux(&stack);
    let shouted = HOME.to_uppercase();

    for refused_root in [
        "/ETC".to_owned(),
        "/Etc/media".to_owned(),
        "/uSr/media".to_owned(),
        "/LIB64".to_owned(),
        "/library/media".to_owned(),
        "/SYSTEM/media".to_owned(),
        shouted.clone(),
        format!("{shouted}/.SSH"),
        format!("{shouted}/LIBRARY/media"),
        format!("{shouted}/MEDIA/.Cache"),
        "/RUN".to_owned(),
        "/MNT".to_owned(),
        "/users".to_owned(),
        "/HOME/someone/media".to_owned(),
    ] {
        assert!(
            refused(&mac, &refused_root).await,
            "{refused_root} is refused"
        );
    }
    assert!(!refused(&mac, &format!("{shouted}/MEDIA")).await);
    // A name no host has in any case, since the disk the test runs on resolves what it
    // has in its own case.
    assert!(refused(&mac, "/Lib64-lemonfiber-test").await);
    assert!(!refused(&linux, "/Lib64-lemonfiber-test").await);
}

/// Inside a container that has the stack's directory mounted at the host's path, with no
/// home of the operator's and the data root itself not there, the judgement is the same:
/// it reads only the paths.
#[tokio::test]
async fn a_container_with_the_stack_at_the_hosts_path_judges_the_same() {
    let stack = Path::new("/volume1/docker/lemonfiber-test-stack");
    let ctx = ctx(Some(Path::new("/home/op")), stack, Environment::LinuxNative);

    for fine in [
        "./media",
        "/volume1/docker/lemonfiber-test-stack/media",
        "/volume1/media",
        "/tank/media",
    ] {
        assert!(!refused(&ctx, fine).await, "{fine} is taken");
    }
    for refused_root in ["/", "/etc", "/var/media", "/root/media", ".."] {
        assert!(
            refused(&ctx, refused_root).await,
            "{refused_root} is refused"
        );
    }
}

/// With no stack directory there is nothing a relative path could lie inside.
#[tokio::test]
async fn with_no_stack_a_relative_path_is_refused() {
    let stackless = a_context()
        .over(crate::stack::Source::Embedded(&STACKLET))
        .settings(crate::config::Settings {
            home: Some(Path::new(HOME).to_path_buf()),
            ..crate::config::Settings::default()
        })
        .build();

    assert!(refused(&stackless, "./data").await);
    assert!(!refused(&stackless, "/tank/media").await);
}

/// A refusal says why, in words of what would be handed over.
#[tokio::test]
async fn a_refusal_says_what_would_be_handed_over() {
    let (_dir, stack) = machine("data-root-words");
    let ctx = linux(&stack);

    let whole = refusal(&ctx, "/").await.unwrap_or_default();
    let home = refusal(&ctx, HOME).await.unwrap_or_default();

    assert!(whole.contains("the whole machine"), "{whole}");
    assert!(home.contains("holds your home"), "{home}");
}

/// A home, a stack directory or a path inside the stack that leads through a link to
/// somewhere not there yet cannot be judged, so nothing is taken against it; nor is a
/// path inside a stack directory no part of which is there.
#[cfg(unix)]
#[tokio::test]
async fn what_cannot_be_resolved_is_never_taken() {
    let (dir, stack) = machine("data-root-unresolved");
    let dangling_home = dir.join("home");
    let dangling_stack = dir.join("gone-stack");
    let _ = std::os::unix::fs::symlink("/etc/lemonfiber-no-home-yet", &dangling_home);
    let _ = std::os::unix::fs::symlink("/etc/lemonfiber-no-stack-yet", &dangling_stack);
    let _ = std::os::unix::fs::symlink("/etc/lemonfiber-nowhere-yet", stack.join("dangling"));

    let homeless = ctx(Some(&dangling_home), &stack, Environment::LinuxNative);
    let said = refusal(&homeless, "/tank/media").await.unwrap_or_default();
    assert!(said.contains("home directory is not known"), "{said}");

    for (stack, value) in [
        (dangling_stack.as_path(), "./media"),
        (stack.as_path(), "./dangling/media"),
        (Path::new("lemonfiber-no-such-relative-stack"), "./media"),
    ] {
        let said = refusal(&linux(stack), value).await.unwrap_or_default();
        assert!(
            said.contains("has to stay inside"),
            "{}: {said}",
            stack.display()
        );
    }
}
