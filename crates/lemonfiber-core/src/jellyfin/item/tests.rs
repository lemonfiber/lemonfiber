use super::{ItemResource, ItemsResource};
use crate::ports::service::{Holds, Medium};

fn read(json: &str) -> Vec<ItemResource> {
    serde_json::from_str::<ItemsResource>(json)
        .map(|page| page.items)
        .unwrap_or_default()
}

#[test]
fn a_film_with_both_pictures_holds_both_and_plays() {
    let held = read(
        r#"{"Items":[{"Id":"a1","Name":"Heat","ProductionYear":1995,"Type":"Movie",
        "ImageTags":{"Primary":"p"},"BackdropImageTags":["b"],"IsFolder":false}]}"#,
    )
    .into_iter()
    .map(ItemResource::item)
    .next();
    let held = held.map(|held| (held.medium, held.holds, held.year));
    assert_eq!(
        held,
        Some((
            Medium::Film,
            Holds {
                poster: true,
                backdrop: true,
                plays: true
            },
            Some(1995)
        ))
    );
}

#[test]
fn a_series_plays_nothing_itself_and_an_episode_is_named_one() {
    let held: Vec<_> = read(
        r#"{"Items":[{"Id":"s","Name":"The Wire","Type":"Series","IsFolder":true},
        {"Id":"e","Name":"The Target","Type":"Episode","IsFolder":false},
        {"Id":"o","Name":"A Book","Type":"Book"}]}"#,
    )
    .into_iter()
    .map(|item| {
        let series = item.is_a_series();
        let held = item.item();
        (held.medium, held.holds.plays, held.holds.poster, series)
    })
    .collect();
    assert_eq!(
        held,
        vec![
            (Medium::Series, false, false, true),
            (Medium::Episode, true, false, false),
            (Medium::Other, true, false, false),
        ]
    );
}
