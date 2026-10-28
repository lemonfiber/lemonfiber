use lemonfiber_core::model::{Waiting, Wanted};

#[test]
fn each_service_held_back_is_named_with_everything_it_waits_for_and_never_a_value() {
    let held = [
        Waiting {
            service: "tunnel".to_owned(),
            wants: vec![
                Wanted::Setting {
                    setting: "guard:account".to_owned(),
                    choices: None,
                },
                Wanted::Setting {
                    setting: "guard:vpn-type".to_owned(),
                    choices: Some(vec!["wireguard".to_owned(), "openvpn".to_owned()]),
                },
                Wanted::Setting {
                    setting: "guard:forwarding".to_owned(),
                    choices: Some(Vec::new()),
                },
                Wanted::Secret {
                    secret: "guard:wireguard-key".to_owned(),
                },
            ],
        },
        Waiting {
            service: "torrents".to_owned(),
            wants: vec![Wanted::Service {
                service: "tunnel".to_owned(),
            }],
        },
    ];
    assert_eq!(
        super::waiting(&held).text(),
        "\nHeld back until you give what they need to start:\n\
         \x20 tunnel waits for the setting guard:account; the setting guard:vpn-type, one of \
         wireguard, openvpn; the setting guard:forwarding, which has no choice until the \
         settings it depends on change; the secret guard:wireguard-key\n\
         \x20 torrents waits for tunnel, which it runs through\n\
         Give a setting with lemonfiber settings set <setting>=<value>."
    );
    assert_eq!(super::waiting(&[]).text(), "");
    let secret = [Waiting {
        service: "tunnel".to_owned(),
        wants: vec![Wanted::Secret {
            secret: "guard:wireguard-key".to_owned(),
        }],
    }];
    assert_eq!(
        super::waiting(&secret).text(),
        "\nHeld back until you give what they need to start:\n\
         \x20 tunnel waits for the secret guard:wireguard-key"
    );
}
