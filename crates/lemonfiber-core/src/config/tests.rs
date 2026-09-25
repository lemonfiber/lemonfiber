use super::{
    env, front_door_from_env, household_host_from_env, indexer_from_env, ip_echo_from_env,
    provider_host_from_env, Indexer, Protocol, Protocols, Settings, DEFAULT_IP_ECHO, OFFLINE_KEY,
    SECOND_IP_ECHO,
};

#[test]
fn a_fresh_install_has_no_way_to_download_yet() {
    assert!(!Protocols::none().any());
    assert_eq!(Protocols::default(), Protocols::none());
}

#[test]
fn an_indexer_is_read_only_when_both_its_url_and_key_are_present() {
    // Both present: an indexer to re-prove, whitespace trimmed.
    let file = env::EnvFile::parse("INDEXER_URL= http://idx/api \nINDEXER_APIKEY=abc\n");
    assert_eq!(
        indexer_from_env(&file),
        Some(Indexer {
            url: "http://idx/api".to_owned(),
            key: "abc".to_owned(),
        })
    );

    // A URL with no key is half-written, so no indexer to test.
    let url_only = env::EnvFile::parse("INDEXER_URL=http://idx/api\n");
    assert_eq!(indexer_from_env(&url_only), None);

    // A key with no URL is nowhere to send it, so likewise none.
    let key_only = env::EnvFile::parse("INDEXER_APIKEY=abc\n");
    assert_eq!(indexer_from_env(&key_only), None);

    // An empty value counts as absent, not as a blank indexer.
    let empty = env::EnvFile::parse("INDEXER_URL=\nINDEXER_APIKEY=abc\n");
    assert_eq!(indexer_from_env(&empty), None);

    // Nothing configured at all.
    assert_eq!(indexer_from_env(&env::EnvFile::parse("")), None);
}

#[test]
fn the_household_address_is_read_and_a_blank_one_is_absent() {
    // A blank is a mistake, never an intent — and an address of nothing is the
    // one thing worse to hand somebody than no address at all.
    let written = env::EnvFile::parse("HOMEPAGE_VAR_LAN_HOST= 192.168.1.10 \n");
    assert_eq!(
        household_host_from_env(&written),
        Some("192.168.1.10".to_owned())
    );
    let blank = env::EnvFile::parse("HOMEPAGE_VAR_LAN_HOST=\n");
    assert_eq!(household_host_from_env(&blank), None);
    assert_eq!(household_host_from_env(&env::EnvFile::parse("")), None);
}

#[test]
fn the_named_front_door_is_read_and_a_blank_one_is_absent() {
    // The same reading and the same reason: a blank is a mistake, never an
    // intent, and a door named nothing would refuse on every run.
    let written = env::EnvFile::parse("LEMONFIBER_FRONT_DOOR= jellyfin \n");
    assert_eq!(front_door_from_env(&written), Some("jellyfin".to_owned()));
    let blank = env::EnvFile::parse("LEMONFIBER_FRONT_DOOR=\n");
    assert_eq!(front_door_from_env(&blank), None);
    assert_eq!(front_door_from_env(&env::EnvFile::parse("")), None);
    assert_eq!(Settings::default().front_door, None);
}

#[test]
fn either_protocol_alone_counts() {
    let usenet = Protocols {
        usenet: true,
        torrent: false,
    };
    let torrent = Protocols {
        usenet: false,
        torrent: true,
    };
    assert!(usenet.any());
    assert!(torrent.any());
    assert!(Protocols::both().any());
}

#[test]
fn each_protocol_answers_for_itself() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    assert!(usenet_only.has(Protocol::Usenet));
    assert!(!usenet_only.has(Protocol::Torrent));
    assert!(Protocols::both().has(Protocol::Torrent));
    assert!(!Protocols::none().has(Protocol::Usenet));
}

#[test]
fn a_provider_is_configured_only_when_it_says_so() {
    let file = env::EnvFile::parse("LEMONFIBER_USENET=on\nLEMONFIBER_TORRENT=off\n");
    assert_eq!(
        Protocols::from_env(&file),
        Protocols {
            usenet: true,
            torrent: false
        }
    );
}

#[test]
fn nothing_recorded_means_nothing_configured() {
    assert_eq!(
        Protocols::from_env(&env::EnvFile::parse("")),
        Protocols::none()
    );
}

#[test]
fn a_setting_may_be_spelled_the_ways_people_spell_it() {
    // Including quoted, which a hand-edited .env commonly is.
    for on in ["on", "ON", "true", "yes", "1", " on ", "\"on\"", "'yes'"] {
        assert!(super::reads_as_on(on), "{on:?} should read as on");
    }
    for off in ["off", "false", "no", "0", "", "maybe", "\"off\"", "\"\""] {
        assert!(!super::reads_as_on(off), "{off:?} should not read as on");
        if off != "maybe" {
            assert!(super::reads_as_off(off), "{off:?} should read as off");
        }
    }
}

#[test]
fn the_project_name_defaults_to_the_product() {
    let settings = Settings::default();
    assert_eq!(settings.project, "lemonfiber");
    assert_eq!(settings.env_file, None);
    assert_eq!(settings.stack_dir, None);
    assert!(settings.overlays.is_empty());
}

#[test]
fn leak_detection_is_on_by_default() {
    // The failure it catches reaches outside the machine, so a fresh install
    // is protected without the operator having to ask.
    // Two sources, not one: the whole verdict is a comparison against what
    // they report, and a single stranger who is wrong makes the check say
    // `pass` while traffic leaves in the clear.
    assert_eq!(
        ip_echo_from_env(&env::EnvFile::parse("")),
        vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()]
    );
    assert_eq!(
        Settings::default().ip_echo,
        vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()]
    );
}

#[test]
fn an_operator_can_switch_leak_detection_off() {
    for off in ["off", "OFF", "no", "false", "0", ""] {
        let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={off}\n"));
        assert!(
            ip_echo_from_env(&file).is_empty(),
            "{off:?} should disable it"
        );
    }
}

/// The one setting here that names a thing as well as answering yes or no, so
/// the blanket switch has to be read where it is read rather than beside the
/// four that only answer.
#[test]
fn the_blanket_switch_stops_the_leak_check_even_where_a_source_is_named() {
    let file = env::EnvFile::parse(&format!(
        "{OFFLINE_KEY}=on\nLEMONFIBER_IP_ECHO=https://ip.example\n"
    ));
    assert!(ip_echo_from_env(&file).is_empty());
}

/// Where the requests that leave this machine are listed, the Usenet provider is
/// named by its host — so the host is read back out of the file, and the account
/// beside it never is.
#[test]
fn a_usenet_host_is_read_back_and_a_blank_one_is_absent() {
    assert_eq!(
        provider_host_from_env(&env::EnvFile::parse("USENET_HOST= news.example.net \n")),
        Some("news.example.net".to_owned())
    );
    for blank in ["USENET_HOST=\n", "USENET_HOST=   \n", "PUID=1000\n"] {
        assert_eq!(
            provider_host_from_env(&env::EnvFile::parse(blank)),
            None,
            "{blank:?}"
        );
    }
}

#[test]
fn an_affirmative_value_leaves_the_default_in_place() {
    for on in ["on", "yes", "true", "1"] {
        let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={on}\n"));
        assert_eq!(
            ip_echo_from_env(&file),
            vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()],
            "{on:?}"
        );
    }
}

#[test]
fn any_other_value_replaces_the_default_endpoint() {
    let file = env::EnvFile::parse("LEMONFIBER_IP_ECHO=https://ip.example\n");
    assert_eq!(
        ip_echo_from_env(&file),
        vec!["https://ip.example".to_owned()]
    );
}

#[test]
fn an_operator_can_name_several_sources_of_their_own() {
    // Naming one must not silently drop back to trusting a single stranger,
    // which is the arrangement asking two exists to avoid.
    let file =
        env::EnvFile::parse("LEMONFIBER_IP_ECHO=https://ip.example, https://other.example\n");
    assert_eq!(
        ip_echo_from_env(&file),
        vec![
            "https://ip.example".to_owned(),
            "https://other.example".to_owned()
        ]
    );
}

#[test]
fn a_hand_quoted_endpoint_loses_the_quotes_but_keeps_its_case() {
    // A person editing the file by hand commonly quotes the value; the
    // quotes must not reach the container's wget, and a URL's path is
    // case-sensitive so the value is not folded like the on/off switch is.
    let file = env::EnvFile::parse("LEMONFIBER_IP_ECHO=\"https://IP.Example/Path\"\n");
    assert_eq!(
        ip_echo_from_env(&file),
        vec!["https://IP.Example/Path".to_owned()]
    );
}

#[test]
fn a_data_root_is_read_when_set_and_absent_when_blank() {
    use std::path::PathBuf;

    assert_eq!(
        super::data_root_from_env(&env::EnvFile::parse("DATA_ROOT=/srv/media\n")),
        Some(PathBuf::from("/srv/media"))
    );
    // A blank or unset value is not the current directory; it is no choice
    // yet, and the storage check treats it as such.
    assert_eq!(
        super::data_root_from_env(&env::EnvFile::parse("DATA_ROOT=   \n")),
        None
    );
    assert_eq!(super::data_root_from_env(&env::EnvFile::parse("")), None);
    assert_eq!(Settings::default().data_root, None);
}

#[test]
fn a_service_user_is_read_only_when_both_halves_are_present_and_numeric() {
    assert_eq!(
        super::service_user_from_env(&env::EnvFile::parse("PUID=1000\nPGID=1001\n")),
        Some((1000, 1001))
    );
    // One half without the other cannot answer the question it is for, so it
    // is treated as unconfigured rather than half-guessed.
    assert_eq!(
        super::service_user_from_env(&env::EnvFile::parse("PUID=1000\n")),
        None
    );
    assert_eq!(
        super::service_user_from_env(&env::EnvFile::parse("PUID=root\nPGID=1000\n")),
        None
    );
    assert_eq!(Settings::default().service_user, None);
}

#[test]
fn port_forwarding_reads_the_switch_and_the_provider() {
    let file = env::EnvFile::parse("VPN_PROVIDER=ProtonVPN\nVPN_PORT_FORWARDING=on\n");
    let recorded = super::port_forward_from_env(&file);
    assert!(recorded.enabled);
    // The provider is lower-cased so the check can match it without caring how
    // the operator spelled it.
    assert_eq!(recorded.provider.as_deref(), Some("protonvpn"));
}

#[test]
fn a_quoted_provider_is_de_quoted_like_the_switch() {
    // A hand-edited .env commonly quotes values. The provider must be stripped
    // the same way the switch is, or a quoted name misses its known trap and
    // reads as an unknown provider.
    let file = env::EnvFile::parse("VPN_PROVIDER=\"protonvpn\"\nVPN_PORT_FORWARDING=\"on\"\n");
    let recorded = super::port_forward_from_env(&file);
    assert!(recorded.enabled);
    assert_eq!(recorded.provider.as_deref(), Some("protonvpn"));
}

#[test]
fn port_forwarding_is_off_by_default_and_a_blank_provider_is_absent() {
    // A fresh install has no VPN configured, so nothing to verify a port for.
    assert!(!super::port_forward_from_env(&env::EnvFile::parse("")).enabled);
    assert_eq!(
        Settings::default().port_forward,
        super::PortForward::default()
    );

    // A named-but-empty provider is no provider, not one called "".
    let blank = env::EnvFile::parse("VPN_PROVIDER=   \nVPN_PORT_FORWARDING=off\n");
    let recorded = super::port_forward_from_env(&blank);
    assert!(!recorded.enabled);
    assert_eq!(recorded.provider, None);
}
