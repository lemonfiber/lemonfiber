use super::{
    Addressee, Answers, BytePath, Declared, Held, LivePath, Locations, Naming, Ports, Presentation,
    Question, Statuses, Token, Undeclarable,
};
use crate::http::Method;

/// A declaration a guard can enforce, serving bytes under one path.
fn declared() -> Declared {
    Declared {
        bytes: vec![BytePath {
            pattern: "^/media/(?P<item>[^/]+)/bytes$".to_owned(),
            item: "^[0-9]+$".to_owned(),
            exempt: vec!["^/media/now/bytes$".to_owned()],
            naming: vec![
                Naming {
                    parameter: "other".to_owned(),
                    held: Held::Refused,
                },
                Naming {
                    parameter: "also".to_owned(),
                    held: Held::Asked {
                        shape: "^[0-9]+$".to_owned(),
                    },
                },
            ],
        }],
        live: vec![LivePath {
            pattern: "^/media/live/(?P<stream>[^/]+)/".to_owned(),
            stream: "^[0-9]+$".to_owned(),
        }],
        refused: vec!["^/everything$".to_owned()],
        question: Some(Question {
            method: Method::Get,
            path: "/media/{item}".to_owned(),
            to: Addressee::Upstream,
            answers: Answers {
                may: vec![Statuses { from: 200, to: 299 }],
                may_not: vec![Statuses::only(404)],
                no_session: vec![Statuses::only(401)],
                not_from_there: vec![Statuses::only(403)],
            },
        }),
        token: Token {
            presented: Presentation::AsIs,
            shape: "^[0-9]+$".to_owned(),
            parameters: vec!["key".to_owned()],
        },
        locations: Locations {
            stream: "{base}/media/{item}/bytes".to_owned(),
            poster: "{base}/media/{item}/poster".to_owned(),
            backdrop: "{base}/media/{item}/backdrop".to_owned(),
        },
        ports: Ports {
            upstream: 8080,
            encrypted: None,
            internal_tls: None,
        },
    }
}

#[test]
fn a_declaration_a_guard_can_enforce_is_kept_whole() {
    assert_eq!(declared().checked(), Ok(declared()));
}

#[test]
fn a_server_serving_no_bytes_needs_no_question() {
    let unguarded = Declared {
        bytes: Vec::new(),
        question: None,
        ..declared()
    };
    assert_eq!(unguarded.clone().checked(), Ok(unguarded));
}

#[test]
fn bytes_served_with_no_question_are_refused_naming_the_path() {
    let unasked = Declared {
        question: None,
        ..declared()
    };
    assert_eq!(
        unasked.checked(),
        Err(Undeclarable::Unasked {
            pattern: "^/media/(?P<item>[^/]+)/bytes$".to_owned()
        })
    );
}

#[test]
fn a_byte_path_that_does_not_say_where_the_item_sits_once_is_refused() {
    for pattern in ["^/media/([^/]+)/bytes$", "^/media/(?P<thing>[^/]+)/bytes$"] {
        let mut unplaced = declared();
        if let Some(path) = unplaced.bytes.first_mut() {
            pattern.clone_into(&mut path.pattern);
        }
        assert_eq!(
            unplaced.checked(),
            Err(Undeclarable::Unplaced {
                pattern: pattern.to_owned()
            })
        );
    }
}

#[test]
fn a_question_that_names_no_item_is_refused() {
    let mut unnamed = declared();
    if let Some(question) = unnamed.question.as_mut() {
        question.path = "/media".to_owned();
    }
    assert_eq!(
        unnamed.checked(),
        Err(Undeclarable::Unnamed {
            template: "/media".to_owned()
        })
    );
}

#[test]
fn a_location_built_anywhere_but_on_the_household_address_is_refused() {
    for template in [
        "https://elsewhere.example/media/{item}",
        "{base}.elsewhere.example/media/{item}",
        "{base}@elsewhere.example/media/{item}",
        "/media/{item}?from={base}",
        "{base}/{base}/media/{item}",
    ] {
        for at in 0..3 {
            let mut elsewhere = declared();
            let location = [
                &mut elsewhere.locations.stream,
                &mut elsewhere.locations.poster,
                &mut elsewhere.locations.backdrop,
            ];
            if let Some(location) = location.into_iter().nth(at) {
                template.clone_into(location);
            }
            assert_eq!(
                elsewhere.checked(),
                Err(Undeclarable::Elsewhere {
                    template: template.to_owned()
                }),
                "{template} at {at}"
            );
        }
    }
}

#[test]
fn a_location_that_names_no_item_is_refused() {
    let mut unnamed = declared();
    unnamed.locations.backdrop = "{base}/media/backdrop".to_owned();
    assert_eq!(
        unnamed.checked(),
        Err(Undeclarable::Unnamed {
            template: "{base}/media/backdrop".to_owned()
        })
    );
}

#[test]
fn a_declaration_crosses_as_it_is_written_and_refuses_a_field_it_does_not_hold() {
    let written = serde_json::to_value(declared()).unwrap_or_default();
    for (pointer, value) in [
        ("/question/method", "get"),
        ("/bytes/0/naming/0/held", "refused"),
        ("/token/presented", "as-is"),
    ] {
        assert_eq!(
            written.pointer(pointer).and_then(serde_json::Value::as_str),
            Some(value),
            "{pointer}"
        );
    }
    let read: Result<Declared, _> = serde_json::from_value(written.clone());
    assert_eq!(read.ok(), Some(declared()));
    let mut more = written;
    if let Some(fields) = more.as_object_mut() {
        fields.insert("also".to_owned(), serde_json::Value::Bool(true));
    }
    assert!(serde_json::from_value::<Declared>(more).is_err());
}

#[test]
fn a_header_presentation_names_its_header_and_its_template() {
    let header = Presentation::Header {
        name: "Authorization".to_owned(),
        value: "Scheme {token}".to_owned(),
    };
    let written = serde_json::to_value(&header).unwrap_or_default();
    for (pointer, value) in [
        ("/header/name", "Authorization"),
        ("/header/value", "Scheme {token}"),
    ] {
        assert_eq!(
            written.pointer(pointer).and_then(serde_json::Value::as_str),
            Some(value),
            "{pointer}"
        );
    }
}

/// `declared()` with `change` made to it.
fn with(change: impl FnOnce(&mut Declared)) -> Declared {
    let mut changed = declared();
    change(&mut changed);
    changed
}

/// `declared()` asking `path` as its question.
fn asking(path: &str) -> Declared {
    with(|declared| {
        if let Some(question) = declared.question.as_mut() {
            path.clone_into(&mut question.path);
        }
    })
}

#[test]
fn a_question_that_could_be_sent_anywhere_but_a_path_on_its_addressee_is_refused() {
    for path in [
        "Items/{item}",
        "//elsewhere.example/{item}",
        "http://elsewhere.example/{item}",
        "/@elsewhere.example/{item}",
        "/items\\{item}",
        "/items/%2F/{item}",
        "/items/%2e%2e/{item}",
        "/items/../{item}",
        "/items/./{item}",
        "/items//{item}",
        "/items/{item}?also=1",
        "/items/{item}#there",
        "/items/x{item}",
        "/items/{item}.json",
        "/items/{other}/{item}",
        "/items/\r\nHost: elsewhere/{item}",
        "/items/{item}/ ",
    ] {
        assert_eq!(
            asking(path).checked(),
            Err(Undeclarable::Unsent {
                path: path.to_owned()
            }),
            "{path}"
        );
    }
}

#[test]
fn a_question_naming_the_item_as_a_whole_segment_on_a_plain_path_is_kept() {
    for path in [
        "/{item}",
        "/items/{item}",
        "/lemonfiber/media.serve/v1/may/{item}/now",
    ] {
        assert!(asking(path).checked().is_ok(), "{path}");
    }
}

#[test]
fn a_pattern_not_anchored_at_the_paths_start_is_refused() {
    let byte_path = "/media/(?P<item>[^/]+)/bytes$";
    let exempt = "(?i)/media/now/bytes$";
    let refused = ".*/everything$";
    let unanchored = [
        (
            byte_path,
            with(|declared| {
                if let Some(path) = declared.bytes.first_mut() {
                    byte_path.clone_into(&mut path.pattern);
                }
            }),
        ),
        (
            exempt,
            with(|declared| {
                if let Some(path) = declared.bytes.first_mut() {
                    path.exempt = vec![exempt.to_owned()];
                }
            }),
        ),
        (
            refused,
            with(|declared| declared.refused = vec![refused.to_owned()]),
        ),
    ];
    for (pattern, declared) in unanchored {
        assert_eq!(
            declared.checked(),
            Err(Undeclarable::Unanchored {
                pattern: pattern.to_owned()
            }),
            "{pattern}"
        );
    }
}

#[test]
fn a_shape_not_anchored_at_both_ends_is_refused() {
    for shape in ["[0-9]+", "^[0-9]+", "[0-9]+$", "^[0-9]+\\$", "(?i)[0-9]+$"] {
        let item = with(|declared| {
            if let Some(path) = declared.bytes.first_mut() {
                shape.clone_into(&mut path.item);
            }
        });
        let asked = with(|declared| {
            if let Some(path) = declared.bytes.first_mut() {
                path.naming = vec![Naming {
                    parameter: "also".to_owned(),
                    held: Held::Asked {
                        shape: shape.to_owned(),
                    },
                }];
            }
        });
        let token = with(|declared| shape.clone_into(&mut declared.token.shape));
        for unanchored in [item, asked, token] {
            assert_eq!(
                unanchored.checked(),
                Err(Undeclarable::Unanchored {
                    pattern: shape.to_owned()
                }),
                "{shape}"
            );
        }
    }
    let caseless = with(|declared| "(?i)^[a-f]+$".clone_into(&mut declared.token.shape));
    assert!(caseless.checked().is_ok());
}

#[test]
fn a_parameter_that_is_not_a_plain_name_is_refused() {
    for name in ["", "item Id", "item%49d", "item[0]", "a&b=c", "ité"] {
        let naming = with(|declared| {
            if let Some(path) = declared.bytes.first_mut() {
                path.naming = vec![Naming {
                    parameter: name.to_owned(),
                    held: Held::Refused,
                }];
            }
        });
        let carried = with(|declared| declared.token.parameters = vec![name.to_owned()]);
        let presented = with(|declared| {
            declared.token.presented = Presentation::Query {
                parameter: name.to_owned(),
            };
        });
        for unspelled in [naming, carried, presented] {
            assert_eq!(
                unspelled.checked(),
                Err(Undeclarable::Unspelled {
                    name: name.to_owned()
                }),
                "{name:?}"
            );
        }
    }
}

#[test]
fn a_token_presented_in_a_header_a_guard_may_not_write_is_refused() {
    for (name, value) in [
        ("Host", "{token}"),
        ("host", "{token}"),
        ("Connection", "{token}"),
        ("Transfer-Encoding", "{token}"),
        ("Proxy-Authorization", "{token}"),
        ("X-Forwarded-For", "{token}"),
        ("Content-Length", "{token}"),
        ("Authorization: x", "{token}"),
        ("", "{token}"),
        ("Authorization", "Scheme {token}\r\nHost: elsewhere"),
        ("Authorization", "Scheme {token}\n"),
        ("Authorization", "Scheme"),
        ("Authorization", "{token} {token}"),
    ] {
        let presented = with(|declared| {
            declared.token.presented = Presentation::Header {
                name: name.to_owned(),
                value: value.to_owned(),
            };
        });
        assert_eq!(
            presented.checked(),
            Err(Undeclarable::Unpresentable {
                header: name.to_owned()
            }),
            "{name}: {value:?}"
        );
    }
}

#[test]
fn answers_that_are_not_statuses_or_that_overlap_are_refused() {
    for (may, may_not) in [
        (Statuses { from: 200, to: 299 }, Statuses::only(250)),
        (
            Statuses { from: 200, to: 299 },
            Statuses { from: 299, to: 404 },
        ),
        (Statuses { from: 299, to: 200 }, Statuses::only(404)),
        (Statuses { from: 99, to: 200 }, Statuses::only(404)),
        (Statuses { from: 200, to: 600 }, Statuses::only(404)),
    ] {
        let unanswerable = with(|declared| {
            if let Some(question) = declared.question.as_mut() {
                question.answers.may = vec![may];
                question.answers.may_not = vec![may_not];
            }
        });
        assert_eq!(
            unanswerable.checked(),
            Err(Undeclarable::Unanswerable),
            "{may:?} {may_not:?}"
        );
    }
}

#[test]
fn a_location_that_leaves_the_household_address_by_its_path_is_refused() {
    for template in [
        "{base}//elsewhere.example/{item}",
        "{base}/media\\{item}",
        "{base}/media/{item} ",
        "{base}/media/{item}\r\n",
    ] {
        let elsewhere = with(|declared| template.clone_into(&mut declared.locations.poster));
        assert_eq!(
            elsewhere.checked(),
            Err(Undeclarable::Elsewhere {
                template: template.to_owned()
            }),
            "{template:?}"
        );
    }
}

#[test]
fn a_pattern_that_does_not_compile_is_too_large_or_is_anchored_on_one_branch_alone_is_refused() {
    let alternated = "^/media/(?P<item>[^/]+)/bytes$|/(?P<also>x)";
    let lined = "(?m)^/media/(?P<item>[^/]+)/bytes$";
    let doubled = "^/(?P<item>a)/(?P<item>b)$";
    let unclosed = "^/media/(?P<item>[^/]+";
    let long = format!("^/(?P<item>{})$", "a".repeat(1100));
    let huge = "^/(?P<item>(?:[a-z]{1000}){1000})$";
    for pattern in [alternated, lined, doubled, unclosed, long.as_str(), huge] {
        let declared = with(|declared| {
            if let Some(path) = declared.bytes.first_mut() {
                pattern.clone_into(&mut path.pattern);
            }
        });
        assert_eq!(
            declared.checked(),
            Err(Undeclarable::Unanchored {
                pattern: pattern.to_owned()
            }),
            "{pattern}"
        );
    }
    for shape in ["^[0-9]+$|[a-z]+", "(?m)^[0-9]+$", "^(?:[0-9]+$)?"] {
        let declared = with(|declared| shape.clone_into(&mut declared.token.shape));
        assert_eq!(
            declared.checked(),
            Err(Undeclarable::Unanchored {
                pattern: shape.to_owned()
            }),
            "{shape}"
        );
    }
}

#[test]
fn a_token_is_presented_only_in_a_header_a_guard_may_set() {
    for name in ["X-Custom", "Cookie", "Referer", "X-Forwarded-Host"] {
        let presented = with(|declared| {
            declared.token.presented = Presentation::Header {
                name: name.to_owned(),
                value: "{token}".to_owned(),
            };
        });
        assert_eq!(
            presented.checked(),
            Err(Undeclarable::Unpresentable {
                header: name.to_owned()
            }),
            "{name}"
        );
    }
    for name in [
        "Authorization",
        "authorization",
        "X-Emby-Token",
        "X-Api-Key",
    ] {
        let presented = with(|declared| {
            declared.token.presented = Presentation::Header {
                name: name.to_owned(),
                value: "Scheme {token}".to_owned(),
            };
        });
        assert!(presented.checked().is_ok(), "{name}");
    }
    let braced = with(|declared| {
        declared.token.presented = Presentation::Header {
            name: "Authorization".to_owned(),
            value: "Scheme {token} {base}".to_owned(),
        };
    });
    assert_eq!(
        braced.checked(),
        Err(Undeclarable::Unpresentable {
            header: "Authorization".to_owned()
        })
    );
}

#[test]
fn a_location_holding_any_placeholder_but_the_household_address_and_the_item_is_refused() {
    for template in ["{base}/media/{item}/{token}", "{base}/media/{item}/{other}"] {
        let elsewhere = with(|declared| template.clone_into(&mut declared.locations.stream));
        assert_eq!(
            elsewhere.checked(),
            Err(Undeclarable::Elsewhere {
                template: template.to_owned()
            }),
            "{template}"
        );
    }
}

#[test]
fn a_location_another_url_reader_would_take_elsewhere_or_rewrite_is_refused() {
    for template in [
        "{base}/\\elsewhere.example/{item}",
        "{base}/media/%2e%2e/{item}",
        "{base}/media/../{item}",
        "{base}/media/\t{item}",
        "{base}/media/\u{ff0e}\u{ff0e}/{item}",
        "{base}/media/{item}#x",
    ] {
        let elsewhere = with(|declared| template.clone_into(&mut declared.locations.backdrop));
        assert_eq!(
            elsewhere.checked(),
            Err(Undeclarable::Elsewhere {
                template: template.to_owned()
            }),
            "{template:?}"
        );
    }
}

#[test]
fn a_live_stream_path_that_does_not_say_where_the_stream_sits_once_is_refused() {
    for pattern in ["^/media/live/([^/]+)/", "^/media/live/(?P<item>[^/]+)/"] {
        let mut unplaced = declared();
        for live in &mut unplaced.live {
            pattern.clone_into(&mut live.pattern);
        }
        assert_eq!(
            unplaced.checked(),
            Err(Undeclarable::Unstreamed {
                pattern: pattern.to_owned()
            }),
            "{pattern}"
        );
    }
}

#[test]
fn a_live_stream_path_or_shape_not_anchored_where_it_is_relied_on_is_refused() {
    let mut loose = declared();
    for live in &mut loose.live {
        "/media/live/(?P<stream>[^/]+)/".clone_into(&mut live.pattern);
    }
    let mut partial = declared();
    for live in &mut partial.live {
        "[0-9]+".clone_into(&mut live.stream);
    }

    assert!(matches!(
        loose.checked(),
        Err(Undeclarable::Unanchored { .. })
    ));
    assert!(matches!(
        partial.checked(),
        Err(Undeclarable::Unanchored { .. })
    ));
}

#[test]
fn a_declaration_naming_no_live_stream_reads_as_serving_none() {
    let mut text = serde_json::to_value(declared()).unwrap_or_default();
    if let Some(fields) = text.as_object_mut() {
        fields.remove("live");
    }
    let read: Option<Declared> = serde_json::from_value(text).ok();

    assert_eq!(read.map(|one| one.live), Some(Vec::new()));
}
