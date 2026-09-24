use super::{warn_before_confirming, Playback, Warning};
use crate::platform::Environment;
use crate::quality::Preset;
use crate::wizard::Library;

#[test]
fn a_docker_media_server_transcodes_in_hardware_only_where_the_platform_can() {
    // Linux reaches the encoder from the container; macOS and Windows do not.
    assert_eq!(
        Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
        Playback::HardwareTranscoding
    );
    assert_eq!(
        Playback::of(Environment::LinuxDesktop, Library::JellyfinDocker),
        Playback::HardwareTranscoding
    );
    assert_eq!(
        Playback::of(Environment::MacOs, Library::JellyfinDocker),
        Playback::SoftwareOnly
    );
    assert_eq!(
        Playback::of(Environment::Windows, Library::JellyfinDocker),
        Playback::SoftwareOnly
    );
}

#[test]
fn native_jellyfin_reaches_the_encoder_wherever_it_runs() {
    // Native mode exists to reach the host encoder the Docker VM cannot.
    for environment in [Environment::MacOs, Environment::Windows] {
        assert_eq!(
            Playback::of(environment, Library::JellyfinNative),
            Playback::HardwareTranscoding
        );
    }
}

#[test]
fn no_media_server_means_there_is_nothing_to_transcode() {
    for environment in [
        Environment::MacOs,
        Environment::Windows,
        Environment::LinuxNative,
        Environment::LinuxDesktop,
        Environment::Unsupported,
    ] {
        assert_eq!(Playback::of(environment, Library::None), Playback::NoServer);
    }
}

#[test]
fn maximum_on_a_software_only_host_is_warned_before_confirmation() {
    let playback = Playback::of(Environment::MacOs, Library::JellyfinDocker);
    assert_eq!(
        warn_before_confirming(Preset::Maximum, playback),
        Some(Warning {
            preset: Preset::Maximum
        })
    );
}

#[test]
fn a_host_that_hardware_transcodes_needs_no_warning() {
    // Linux Docker, and native mode on macOS, both reach the encoder.
    for playback in [
        Playback::of(Environment::LinuxNative, Library::JellyfinDocker),
        Playback::of(Environment::MacOs, Library::JellyfinNative),
    ] {
        assert!(warn_before_confirming(Preset::Maximum, playback).is_none());
    }
}

#[test]
fn a_preset_that_does_not_force_transcoding_is_never_warned() {
    let software_only = Playback::of(Environment::MacOs, Library::JellyfinDocker);
    for preset in [Preset::SpaceSaving, Preset::Balanced, Preset::HighQuality] {
        assert!(
            warn_before_confirming(preset, software_only).is_none(),
            "{preset:?} does not force transcoding and needs no warning",
        );
    }
}

#[test]
fn without_a_media_server_even_maximum_is_not_warned() {
    let playback = Playback::of(Environment::MacOs, Library::None);
    assert!(warn_before_confirming(Preset::Maximum, playback).is_none());
}
