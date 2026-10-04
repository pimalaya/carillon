//! # Google People
//!
//! The Google People backend: one contact group watched through
//! `people.connections.list` and its sync token.
//!
//! The People API has no addressbook, only groups a contact is a member
//! of, so the watched collection is a group (`myContacts` holding every
//! contact the account owns), and membership is read on every person the
//! feed names. The first listing enumerates the contacts and hands back a
//! sync token; every later one only what moved, a deleted contact marked
//! as such. An expired token (HTTP 410) enumerates again, read against
//! the picture.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result, anyhow};
use io_gpeople::v1::{
    client::{GpeopleClientStd, GpeopleClientStdConnectOptions, GpeopleClientStdError},
    rest::{
        contact_groups::{GpeopleGroupField, list::GpeopleContactGroupsListParams},
        people::{
            GpeoplePerson, GpeoplePersonField, connections::list::GpeopleConnectionsListParams,
        },
    },
};
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use secrecy::ExposeSecret;

use crate::{
    config::{GpeopleConfig, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    picture::{self, Item, Known},
    poll,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How many contacts one listing page asks for, the API's ceiling.
const PERSON_PAGE: u32 = 1000;
/// What a listing reads of a person: its groups, and whether it is gone.
const PERSON_FIELDS: &[GpeoplePersonField] = &[
    GpeoplePersonField::Memberships,
    GpeoplePersonField::Metadata,
];
/// The prefix a contact group's resource name carries.
const GROUP_PREFIX: &str = "contactGroups/";
/// The collection naming every contact the account owns, whatever its
/// groups: the one address book a listing of the account gives.
const EVERY_CONTACT: &str = "contacts";

/// Opens a connection to the People API, resolving the token through
/// `resolver`.
pub fn open(config: &GpeopleConfig, resolver: &mut SecretResolver) -> Result<GpeopleClientStd> {
    let alpn = config
        .alpn
        .clone()
        .unwrap_or_else(|| vec![String::from("http/1.1")]);
    let opts = GpeopleClientStdConnectOptions {
        tls: config.tls.clone().into_tls(alpn),
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
    };
    let token = resolver.resolve(config.auth.token.clone())?;

    debug!("opening gpeople connection");

    Ok(GpeopleClientStd::connect(token.expose_secret(), opts)?)
}

/// Opens the connection and resolves the group, which proves the
/// transport, the token and that the group exists.
pub fn probe(config: &GpeopleConfig, resolver: &mut SecretResolver) -> Result<()> {
    let mut client = open(config, resolver)?;

    // NOTE: every contact resolves with no request, so a listing page is
    // what proves the token there.
    if resolve_group(&mut client, &config.addressbook)?.is_none() {
        let params = GpeopleConnectionsListParams {
            page_size: Some(1),
            ..Default::default()
        };
        client
            .connections_list(PERSON_FIELDS, &params)
            .context("cannot list google contacts")?;
    }

    Ok(())
}

/// Watches the configured group by polling the sync token, until
/// `shutdown` is set.
pub fn watch(
    config: &GpeopleConfig,
    interval: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut client = open(config, &mut SecretResolver::new())?;
    let mut watched = Watched::arm(&mut client, &config.addressbook)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        // NOTE: the connection sat idle the whole interval, which a
        // server is free to close, and the token may have expired
        // meanwhile, so a failed round is given a fresh connection and a
        // fresh token before the session is given up.
        if let Err(err) = watched.round(&mut client, &mut on_event) {
            debug!("gpeople round failed, reconnecting: {err:#}");
            client = open(config, &mut SecretResolver::new())?;
            watched.round(&mut client, &mut on_event)?;
        }
    }

    Ok(())
}

/// The group the watch holds a picture of.
struct Watched {
    /// The group's resource name, which a membership names it by, or
    /// `None` for every contact.
    group: Option<String>,
    /// What the group holds, as of `token`.
    known: Known,
    /// The sync token the next round reads from.
    token: String,
}

impl Watched {
    /// Resolves the group and reads what it holds, which is what a later
    /// change is a change against.
    fn arm(client: &mut GpeopleClientStd, name: &str) -> Result<Self> {
        let group = resolve_group(client, name)?;
        let (persons, token) = list(client, None)?;

        let mut watched = Self {
            group,
            known: Known::new(),
            token,
        };
        watched.known = watched.members(persons);

        debug!(
            "watching gpeople group `{name}` with {} contacts",
            watched.known.len()
        );

        Ok(watched)
    }

    /// Reads what moved since the token, reports it, and advances it.
    ///
    /// Nothing is reported until every page the round reads has answered,
    /// so a round failing part way leaves the picture and the token where
    /// they were, and can simply be run again.
    fn round(
        &mut self,
        client: &mut GpeopleClientStd,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        let events = match list(client, Some(&self.token)) {
            Ok((persons, token)) => {
                trace!("gpeople changed persons: {}", persons.len());
                self.token = token;
                self.apply(persons)
            }
            // NOTE: an expired token means the server's history no longer
            // reaches that far, so the contacts are enumerated again and
            // read against the picture, the gap reporting only what
            // actually differs.
            Err(err) if is_expired(&err) => {
                debug!("gpeople sync token expired, enumerating again");
                let (persons, token) = list(client, None)?;
                self.token = token;
                let fresh = self.members(persons);
                picture::rebase(&mut self.known, WatchDomain::Card, fresh)
            }
            Err(err) => return Err(err),
        };

        for event in events {
            on_event(event, None);
        }

        Ok(())
    }

    /// Reads changed persons against the picture: a deleted one, or one
    /// that left the group, is a removal.
    fn apply(&mut self, persons: Vec<GpeoplePerson>) -> Vec<WatchEvent> {
        let domain = WatchDomain::Card;
        let mut reported = Vec::new();

        for person in persons {
            let deleted = person
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.deleted)
                .unwrap_or(false);

            if deleted || !self.is_member(&person) {
                let id = person.resource_name;
                reported.extend(picture::remove(&mut self.known, domain, id));
                continue;
            }

            let item = item(&person);
            reported.extend(picture::touch(
                &mut self.known,
                domain,
                person.resource_name,
                item,
            ));
        }

        reported
    }

    /// The members of the group among a full listing.
    fn members(&self, persons: Vec<GpeoplePerson>) -> Known {
        persons
            .into_iter()
            .filter(|person| self.is_member(person))
            .map(|person| {
                let item = item(&person);
                (person.resource_name, item)
            })
            .collect()
    }

    /// Whether a person belongs to the watched group, which every
    /// contact does when no group is watched.
    fn is_member(&self, person: &GpeoplePerson) -> bool {
        let Some(watched) = &self.group else {
            return true;
        };

        person.memberships.iter().any(|membership| {
            membership
                .contact_group_membership
                .as_ref()
                .and_then(|group| group.contact_group_resource_name.as_deref())
                == Some(watched.as_str())
        })
    }
}

/// What the picture keeps of a person: its etag, which an edit moves.
fn item(person: &GpeoplePerson) -> Item {
    Item {
        version: Some(person.etag.clone()).filter(|etag| !etag.is_empty()),
        ..Default::default()
    }
}

/// Reads one listing to its end, every page of it, from `token` or from
/// scratch, with the token the next round reads from.
fn list(
    client: &mut GpeopleClientStd,
    token: Option<&str>,
) -> Result<(Vec<GpeoplePerson>, String)> {
    let mut persons = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let params = GpeopleConnectionsListParams {
            page_size: Some(PERSON_PAGE),
            page_token: page_token.as_deref(),
            request_sync_token: true,
            sync_token: token,
            ..Default::default()
        };
        let page = client
            .connections_list(PERSON_FIELDS, &params)
            .context("cannot list google contacts")?
            .response;

        persons.extend(page.connections);

        match (page.next_page_token, page.next_sync_token) {
            (Some(next), _) => page_token = Some(next),
            (None, Some(next)) => return Ok((persons, next)),
            (None, None) => return Err(anyhow!("The People API returned no sync token")),
        }
    }
}

/// Resolves a contact group by its name, case-insensitively, or by its
/// resource name, with or without the `contactGroups/` prefix; `contacts`
/// names every contact rather than a group, `None`.
fn resolve_group(client: &mut GpeopleClientStd, name: &str) -> Result<Option<String>> {
    if name == EVERY_CONTACT {
        return Ok(None);
    }

    let resource_name = match name.strip_prefix(GROUP_PREFIX) {
        Some(_) => name.to_string(),
        None => format!("{GROUP_PREFIX}{name}"),
    };
    let mut page_token: Option<String> = None;

    loop {
        let params = GpeopleContactGroupsListParams {
            page_token: page_token.as_deref(),
            ..Default::default()
        };
        let page = client
            .contact_groups_list(&[GpeopleGroupField::Name], &params)
            .context("cannot list google contact groups")?
            .response;

        let found = page.contact_groups.into_iter().find(|group| {
            let named = |candidate: &Option<String>| {
                candidate
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            };

            group.resource_name == resource_name
                || named(&group.name)
                || named(&group.formatted_name)
        });

        if let Some(group) = found {
            return Ok(Some(group.resource_name));
        }

        match page.next_page_token {
            Some(next) => page_token = Some(next),
            None => return Err(anyhow!("Contact group `{name}` not found on Google People")),
        }
    }
}

/// Whether a failure is the People API refusing an expired sync token
/// (HTTP 410).
fn is_expired(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<GpeopleClientStdError>(),
            Some(GpeopleClientStdError::Send(send)) if send.status() == Some(410)
        )
    })
}

#[cfg(test)]
mod tests {
    use io_gpeople::v1::rest::people::{
        GpeopleContactGroupMembership, GpeopleMembership, GpeoplePersonMetadata,
    };

    use crate::gpeople::*;

    fn person(id: &str, etag: &str, groups: &[&str]) -> GpeoplePerson {
        let memberships = groups
            .iter()
            .map(|group| GpeopleMembership {
                contact_group_membership: Some(GpeopleContactGroupMembership {
                    contact_group_resource_name: Some(format!("{GROUP_PREFIX}{group}")),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect();

        GpeoplePerson {
            resource_name: format!("people/{id}"),
            etag: String::from(etag),
            memberships,
            ..Default::default()
        }
    }

    fn watched() -> Watched {
        Watched {
            group: Some(String::from("contactGroups/myContacts")),
            known: Known::new(),
            token: String::new(),
        }
    }

    /// `contacts` is the one address book a listing gives, every contact.
    #[test]
    fn every_contact_is_pictured_when_no_group_is_watched() {
        let mut watched = watched();
        watched.group = None;
        let known = watched.members(vec![
            person("a", "1", &["myContacts"]),
            person("b", "1", &[]),
        ]);

        assert_eq!(
            vec!["people/a", "people/b"],
            known.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn only_the_group_s_members_are_pictured() {
        let watched = watched();
        let known = watched.members(vec![
            person("a", "1", &["myContacts"]),
            person("b", "1", &["friends"]),
        ]);

        assert_eq!(vec!["people/a"], known.keys().collect::<Vec<_>>());
    }

    #[test]
    fn leaving_the_group_or_being_deleted_is_a_removal() {
        let mut watched = watched();
        watched.known = watched.members(vec![
            person("a", "1", &["myContacts"]),
            person("b", "1", &["myContacts"]),
        ]);

        let mut deleted = person("b", "2", &[]);
        deleted.metadata = Some(GpeoplePersonMetadata {
            deleted: Some(true),
            ..Default::default()
        });

        let events = watched.apply(vec![person("a", "2", &["friends"]), deleted]);

        assert_eq!(2, events.len());
        assert!(
            events
                .iter()
                .all(|event| matches!(event, WatchEvent::ItemRemoved { .. }))
        );
        assert!(watched.known.is_empty());
    }
}
