//! # Picture
//!
//! What a REST change feed is reconciled against.
//!
//! A REST change feed (a Graph delta link, a Google sync token) names
//! the items that moved and says nothing of what they were, so each
//! poller keeps a [`Known`] picture and reads every item it is handed
//! against it. When the feed expires, the collection is listed again and
//! [`rebase`] reports only what differs, so the gap never reads as a wave
//! of removals.

use std::collections::{BTreeMap, BTreeSet};

use crate::event::{WatchDomain, WatchEvent};

/// What a poller knows of one item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    /// The item's flags under their shared names, which only mail has.
    pub flags: BTreeSet<String>,
    /// The version the source stamps the item with (an etag, a Graph
    /// `changeKey`), where it has one.
    pub version: Option<String>,
}

/// What a poller knows of a collection, by item id.
pub type Known = BTreeMap<String, Item>;

/// Reads one item a change feed named, and reports what moved.
///
/// An unknown item is an arrival. A known one is a flag delta for mail,
/// which is immutable, and an edit for anything else, unless both sides
/// carry the same version: a feed may name an item it did not change.
pub fn touch(known: &mut Known, domain: WatchDomain, id: String, item: Item) -> Vec<WatchEvent> {
    match known.insert(id.clone(), item.clone()) {
        None => vec![WatchEvent::ItemAdded { domain, id }],
        Some(before) => moved(domain, id, &before, &item, true),
    }
}

/// Forgets an item a change feed named as gone, and reports it when it
/// was there.
pub fn remove(known: &mut Known, domain: WatchDomain, id: String) -> Option<WatchEvent> {
    known
        .remove(&id)
        .map(|_| WatchEvent::ItemRemoved { domain, id })
}

/// Replaces the picture with a fresh listing, and reports what differs.
///
/// A listing names every item, touched or not, so an edit is reported
/// only where the version moved.
pub fn rebase(known: &mut Known, domain: WatchDomain, fresh: Known) -> Vec<WatchEvent> {
    let mut events: Vec<WatchEvent> = known
        .keys()
        .filter(|id| !fresh.contains_key(*id))
        .map(|id| WatchEvent::ItemRemoved {
            domain,
            id: id.clone(),
        })
        .collect();

    for (id, item) in &fresh {
        match known.get(id) {
            None => events.push(WatchEvent::ItemAdded {
                domain,
                id: id.clone(),
            }),
            Some(before) => events.extend(moved(domain, id.clone(), before, item, false)),
        }
    }

    *known = fresh;
    events
}

/// What moved between two readings of one item.
fn moved(
    domain: WatchDomain,
    id: String,
    before: &Item,
    after: &Item,
    touched: bool,
) -> Vec<WatchEvent> {
    if domain == WatchDomain::Message {
        let added = after
            .flags
            .difference(&before.flags)
            .map(|flag| WatchEvent::FlagAdded {
                domain,
                id: id.clone(),
                flag: flag.clone(),
            });
        let removed = before
            .flags
            .difference(&after.flags)
            .map(|flag| WatchEvent::FlagRemoved {
                domain,
                id: id.clone(),
                flag: flag.clone(),
            });

        return added.chain(removed).collect();
    }

    let unversioned = touched && after.version.is_none();

    if unversioned || before.version != after.version {
        vec![WatchEvent::ItemChanged { domain, id }]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::picture::*;

    fn item(version: Option<&str>) -> Item {
        Item {
            version: version.map(String::from),
            ..Default::default()
        }
    }

    fn known(items: &[(&str, Option<&str>)]) -> Known {
        items
            .iter()
            .map(|(id, version)| (String::from(*id), item(*version)))
            .collect()
    }

    #[test]
    fn a_fed_item_is_an_arrival_then_an_edit_unless_its_version_held() {
        let mut picture = Known::new();
        let card = WatchDomain::Card;

        let events = touch(&mut picture, card, String::from("A"), item(Some("1")));
        assert!(matches!(events[..], [WatchEvent::ItemAdded { .. }]));

        let events = touch(&mut picture, card, String::from("A"), item(Some("1")));
        assert!(events.is_empty());

        let events = touch(&mut picture, card, String::from("A"), item(Some("2")));
        assert!(matches!(events[..], [WatchEvent::ItemChanged { .. }]));

        // NOTE: a feed carrying no version names only what it touched.
        let mut picture = known(&[("B", None)]);
        let events = touch(&mut picture, card, String::from("B"), item(None));
        assert!(matches!(events[..], [WatchEvent::ItemChanged { .. }]));
    }

    #[test]
    fn a_fed_message_reports_its_flags_and_never_an_edit() {
        let mut picture = Known::new();
        let mail = WatchDomain::Message;
        let seen = Item {
            flags: BTreeSet::from([String::from("Seen")]),
            version: None,
        };

        touch(&mut picture, mail, String::from("M"), Item::default());
        let events = touch(&mut picture, mail, String::from("M"), seen);
        assert_eq!(
            vec![WatchEvent::FlagAdded {
                domain: mail,
                id: String::from("M"),
                flag: String::from("Seen"),
            }],
            events
        );

        let events = touch(&mut picture, mail, String::from("M"), Item::default());
        assert!(matches!(events[..], [WatchEvent::FlagRemoved { .. }]));
    }

    /// The reason a rebase exists: an expired feed listed again must not
    /// read as every item edited, nor the gap as removals.
    #[test]
    fn a_rebase_reports_only_what_differs() {
        let mut picture = known(&[("A", Some("1")), ("B", Some("1")), ("C", None)]);
        let fresh = known(&[("A", Some("1")), ("B", Some("2")), ("C", None), ("D", None)]);

        let events = rebase(&mut picture, WatchDomain::Event, fresh.clone());

        assert_eq!(
            vec![
                WatchEvent::ItemChanged {
                    domain: WatchDomain::Event,
                    id: String::from("B"),
                },
                WatchEvent::ItemAdded {
                    domain: WatchDomain::Event,
                    id: String::from("D"),
                },
            ],
            events
        );
        assert_eq!(fresh, picture);

        let events = rebase(&mut picture, WatchDomain::Event, known(&[("A", Some("1"))]));
        assert_eq!(3, events.len());
        assert!(
            events
                .iter()
                .all(|event| matches!(event, WatchEvent::ItemRemoved { .. }))
        );
    }

    #[test]
    fn a_gone_item_is_reported_only_when_it_was_known() {
        let mut picture = known(&[("A", None)]);

        assert!(remove(&mut picture, WatchDomain::Card, String::from("Z")).is_none());
        assert!(remove(&mut picture, WatchDomain::Card, String::from("A")).is_some());
        assert!(picture.is_empty());
    }
}
