use crate::refusing::refusals;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// The whole fixture with one more step, to a host outside the stack, and `reaches`.
fn reaching(reaches: &str) -> Vec<String> {
    let step = "[[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"";
    assert!(WHOLE.contains(step));
    let text = WHOLE.replace(
        step,
        &format!(
            "[[recipe.step]]\nid   = \"cover\"\ncall = {{ method = \"GET\", to = \"covers.example.com\", path = \"/cover\" }}\n\n{step}\n\n{reaches}"
        ),
    );
    Manifest::from_toml(&text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .filter(|said| said.contains("reach["))
                .collect()
        },
    )
}

#[test]
fn a_reach_naming_a_host_a_recipe_calls_outside_the_stack_is_read() {
    let said = reaching(
        "[[reach]]\nhost  = \"covers.example.com\"\nstops = \"New comics arrive without covers.\"",
    );
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn a_reach_naming_a_service_of_its_own_or_a_host_nothing_calls_is_refused_naming_it() {
    for host in ["komga", "posters.example.com"] {
        let said = reaching(&format!(
            "[[reach]]\nhost  = \"{host}\"\nstops = \"Something.\""
        ));
        assert!(
            said.iter()
                .any(|one| one.contains("reach[0].host") && one.contains(host)),
            "{said:?}"
        );
    }
}

#[test]
fn a_host_named_twice_or_a_blank_stops_line_is_refused() {
    let said = reaching(
        "[[reach]]\nhost  = \"covers.example.com\"\nstops = \"No covers.\"\n\n[[reach]]\nhost  = \"covers.example.com\"\nstops = \" \"",
    );
    assert!(
        said.iter()
            .any(|one| one.contains("reach[1].host") && one.contains("earlier")),
        "{said:?}"
    );
    assert!(
        said.iter().any(|one| one.contains("reach[1].stops")),
        "{said:?}"
    );
}
