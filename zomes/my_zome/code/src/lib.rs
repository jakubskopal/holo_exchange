#[macro_use]
extern crate hdk;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate holochain_json_derive;
extern crate regex;

use hdk::{
    AGENT_ADDRESS,
    error::ZomeApiResult,
    entry_definition::ValidatingEntryType,
    holochain_persistence_api::{
        cas::content::Address,
    },
    holochain_json_api::{
        json::JsonString,
        json::RawString,
        error::JsonError,
    },
    holochain_core_types::{
        dna::entry_types::Sharing,
        entry::Entry,
        entry::AppEntryValue,
        link::LinkMatch
    },
    error::ZomeApiError
};

use std::convert::TryFrom;
use serde::export::fmt::Debug;
use serde::Serialize;


// ============================================================================ ZOME DEFINITION

define_zome! {
    entries: [
        anchor_definition(),
        profile_definition(),
        item_definition(),
        exchange_definition()
    ]

    init: || {
        Ok(())
    }

    validate_agent: |_validation_data : EntryValidationData::<AgentId>| {
        Ok(())
    }

    functions: [
        create_profile: {
            inputs: |nickname: String|,
            outputs: |result: ZomeApiResult<Address>|,
            handler: handle_create_profile
        }
        get_my_profile: {
            inputs: | |,
            outputs: |result: ZomeApiResult<EntryWithAddress<Profile>>|,
            handler: handle_get_my_profile
        }
        create_exchange: {
            inputs: |iam_offering: String, iam_requesting: String|,
            outputs: |result: ZomeApiResult<Address>|,
            handler: handle_create_exchange
        }
        find_exchanges: {
            inputs: |iam_offering: String, iam_requesting: String|,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Exchange>>>|,
            handler: handle_find_exchanges
        }
        find_profiles: {
            inputs: |nickname_prefix: String|,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Profile>>>|,
            handler: handle_find_profiles
        }
        find_swaps: {
            inputs: |iam_offering: String, iam_requesting: String, max_swaps: i32|,
            outputs: |result: ZomeApiResult<Vec<Vec<EntryWithAddress<Exchange>>>>|,
            handler: handle_find_swaps
        }
    ]
    traits: {
        hc_public [create_profile, get_my_profile, create_exchange, find_exchanges, find_profiles, find_swaps]
    }
}

fn handle_get_my_profile() -> ZomeApiResult<EntryWithAddress<Profile>> {
    let profile_address = get_my_profile_address()?;

    get_entry_as_type_with_address(profile_address)
}

fn handle_find_profiles(nickname_prefix: String) -> ZomeApiResult<Vec<EntryWithAddress<Profile>>> {
    let profiles_anchor_address = get_anchor_address("profiles")?;

    get_links_and_load_type_with_address::<Profile>(&profiles_anchor_address, LinkMatch::Exactly("anchor_profile"), LinkMatch::Regex(&format!("^{}.*", &regex::escape(&nickname_prefix))))
}

fn handle_find_exchanges(iam_offering: String, iam_requesting: String) -> ZomeApiResult<Vec<EntryWithAddress<Exchange>>> {
    // TODO: rewrite to use the anchors
    let exchanges_anchor_address = get_anchor_address("exchanges")?;

    let exchanges = get_links_and_load_type_with_address::<Exchange>(&exchanges_anchor_address, LinkMatch::Exactly("anchor_exchange"), LinkMatch::Any)?
        .iter()
        .filter(|&p| {
            (iam_offering == "" || iam_offering == p.entry.requesting) && (iam_requesting == "" || iam_requesting == p.entry.offering)
        })
        .map(|e| { e.clone() })
        .collect();

    Ok(exchanges)
}

fn handle_find_swaps_0(iam_offering: String,
                       iam_requesting: String,
                       max_swaps: i32,
                       already_exchanged: Vec<EntryWithAddress<Exchange>>,
                       results: &mut Vec<Vec<EntryWithAddress<Exchange>>>) -> ZomeApiResult<()> {
    if max_swaps == 0 {
        Ok(())
    } else if iam_requesting == "" {
        handle_find_exchanges(iam_offering, "".into())?
            .iter()
            .for_each(|ex| {
                let entry = [ already_exchanged.as_slice(), vec![ex.clone()].as_slice() ].concat();
                results.push(entry.clone());
                handle_find_swaps_0(ex.entry.offering.clone(), "".into(), max_swaps - 1, entry, results)
                    .unwrap_or(())
            });
        Ok(())
    } else if iam_offering == "" {
        handle_find_exchanges("".into(), iam_requesting)?
            .iter()
            .for_each(|ex| {
                let entry = [ vec![ex.clone()].as_slice(), already_exchanged.as_slice() ].concat();
                results.push(entry.clone());
                handle_find_swaps_0("".into(), ex.entry.requesting.clone(), max_swaps - 1, entry, results)
                    .unwrap_or(())
            });
        Ok(())
    } else {
        Err(ZomeApiError::Internal("Not implemented".into()))
    }
}

fn handle_find_swaps(iam_offering: String, iam_requesting: String, max_swaps: i32) -> ZomeApiResult<Vec<Vec<EntryWithAddress<Exchange>>>>{
    let mut result = Vec::new();
    handle_find_swaps_0(iam_offering, iam_requesting, max_swaps, vec![], &mut result)?;
    Ok(result.clone())
}

fn handle_create_exchange(offering: String, requesting: String) -> ZomeApiResult<Address> {
    let offered_item = get_or_create_item(offering.clone())?;
    let requested_item = get_or_create_item(requesting.clone())?;
    let profile = get_my_profile_address()?;

    let exchanges_anchor_address = get_anchor_address("exchanges")?;

    let exchange = Entry::App(
        "exchange".into(),
        Exchange {
            requesting: requesting.clone(),
            offering: offering.clone(),
            profile: profile.to_string().into()
        }.into()
    );

    let exchange_address = hdk::commit_entry(&exchange)?;

    hdk::link_entries(&exchanges_anchor_address, &exchange_address, "anchor_exchange", "")?;
    hdk::link_entries(&offered_item, &exchange_address, "item_exchange", "offering")?;
    hdk::link_entries(&requested_item, &exchange_address, "item_exchange", "requesting")?;
    hdk::link_entries(&profile, &exchange_address, "profile_exchange", "")?;

    Ok(exchange_address)
}

fn handle_create_profile(nickname: String) -> ZomeApiResult<Address> {
    let profiles_anchor_address = get_anchor_address("profiles")?;

    let existing_users = hdk::get_links(&profiles_anchor_address, LinkMatch::Exactly("anchor_profile"), LinkMatch::Exactly(&nickname))?
        .addresses()
        .len();

    if existing_users > 0 {
        return Err(ZomeApiError::Internal("Profile already exists".into()))
    }

    let profile = Entry::App(
        "profile".into(),
        Profile {
            nickname: nickname.clone(),
            address: AGENT_ADDRESS.to_string().into()
        }.into()
    );

    let profile_address = hdk::commit_entry(&profile)?;

    hdk::link_entries(&profiles_anchor_address, &profile_address, "anchor_profile".into(), nickname.clone())?;

    let existing_users_again = hdk::get_links(&profiles_anchor_address, LinkMatch::Exactly("anchor_profile"), LinkMatch::Exactly(&nickname))?
        .addresses()
        .len();

    if existing_users_again > 1 {
        hdk::remove_link(&profiles_anchor_address, &profile_address, "anchor_profile".into(), nickname.clone())?;
        return Err(ZomeApiError::Internal("Profile already exists".into()))
    }


    hdk::link_entries(&AGENT_ADDRESS, &profile_address, "agent_profile".into(), "")?;

    Ok(profile_address)
}

// ============================================================================ ANCHOR

pub fn anchor_definition() -> ValidatingEntryType {
    entry!(
        name: "anchor",
        description: "",
        sharing: Sharing::Public,
        validation_package: || hdk::ValidationPackageDefinition::Entry,
        validation: |_validation_data: hdk::EntryValidationData<RawString>| {
            Ok(())
        },
        links: [
            to!(
                "profile",
                link_type: "anchor_profile",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            to!(
                "item",
                link_type: "anchor_item",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            to!(
                "exchange",
                link_type: "anchor_exchange",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            )
        ]
    )
}

fn get_anchor_address(name: &'static str) -> ZomeApiResult<Address> {
    let exchanges_anchor = Entry::App(
        "anchor".into(),
        RawString::from(name).into()
    );

    hdk::commit_entry(&exchanges_anchor)
}

// ============================================================================ PROFILE

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Profile {
    nickname: String,
    address: Address
}

pub fn profile_definition() -> ValidatingEntryType {
    entry!(
        name: "profile",
        description: "",
        sharing: Sharing::Public,
        validation_package: || hdk::ValidationPackageDefinition::Entry,
        validation: |_validation_data: hdk::EntryValidationData<Profile>| {
            Ok(())
        },
        links: [
            from!(
                "%agent_id",
                link_type: "agent_profile",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            ),
            to!(
                "exchange",
                link_type: "profile_exchange",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            )
        ]
    )
}

fn get_my_profile_address() -> ZomeApiResult<Address> {
    let address = hdk::get_links(&AGENT_ADDRESS, LinkMatch::Exactly("agent_profile"), LinkMatch::Any)?.addresses()
        .iter()
        .next()
        .ok_or(ZomeApiError::Internal("We do not seem to have a profile".into()))?
        .clone();

    Ok(address)
}

// ============================================================================ ITEM

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Item {
    name: String,
}

pub fn item_definition() -> ValidatingEntryType {
    entry!(
        name: "item",
        description: "",
        sharing: Sharing::Public,
        validation_package: || hdk::ValidationPackageDefinition::Entry,
        validation: |_validation_data: hdk::EntryValidationData<Item>| {
            Ok(())
        },
        links: [
            to!(
                "exchange",
                link_type: "item_exchange",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            )
        ]
    )
}

fn get_or_create_item(name: String) -> ZomeApiResult<Address> {
    let items_anchor_address = get_anchor_address("items")?;

    let item = Entry::App(
        "item".into(),
        Item {
            name: name.clone()
        }.into()
    );

    let item_address = hdk::commit_entry(&item)?;

    hdk::link_entries(&items_anchor_address, &item_address, "anchor_item".into(), name.clone())?;
    Ok(item_address)
}

// ============================================================================ EXCHANGE

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Exchange {
    requesting: String,
    offering: String,
    profile: Address
}

pub fn exchange_definition() -> ValidatingEntryType {
    entry!(
        name: "exchange",
        description: "",
        sharing: Sharing::Public,
        validation_package: || hdk::ValidationPackageDefinition::Entry,
        validation: |_validation_data: hdk::EntryValidationData<Exchange>| {
            Ok(())
        },
        links: [
        ]
    )
}

// ============================================================================ ENTRY WITH ADDRESS UTILITY

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntryWithAddress<T: Debug + Serialize + Clone> {
    address: Address,
    entry: T
}

impl <T: Debug + Serialize + Clone> From<EntryWithAddress<T>> for JsonString {
    fn from(v: EntryWithAddress<T>) -> Self {
        JsonString::from_json(
            &format!(r#"{{ "address": {}, "entry": {} }}"#,
                     serde_json::to_string(&v.address).unwrap_or_else(|_| panic!("could not Jsonify: {:?}", v)),
                     serde_json::to_string(&v.entry).unwrap_or_else(|_| panic!("could not Jsonify: {:?}", v))
            ))
    }
}

///
/// Helper function that perfoms a try_from for every entry
/// of a get_links_and_load for a given type. Any entries that either fail to
/// load or cannot be converted to the type will be dropped.
///
pub fn get_links_and_load_type_with_address<R: TryFrom<AppEntryValue> + Debug + Serialize + Clone>(
    base: &Address,
    link_type: LinkMatch<&str>,
    tag: LinkMatch<&str>,
) -> ZomeApiResult<Vec<EntryWithAddress<R>>> {
    let link_load_results = hdk::get_links_and_load(base, link_type, tag)?;

    let results_with_errors_too = link_load_results
        .iter()
        .map(|maybe_entry| match maybe_entry {
            Ok(entry) => match entry {
                Entry::App(_, entry_value) => {
                    let typed_entry = R::try_from(entry_value.to_owned()).map_err(|_| {
                        ZomeApiError::Internal(
                            "Could not convert get_links result to requested type".to_string(),
                        )
                    })?;

                    Ok(EntryWithAddress {
                        entry: typed_entry,
                        address: hdk::entry_address(entry)?
                    })
                }
                _ => Err(ZomeApiError::Internal(
                    "get_links did not return an app entry".to_string(),
                )),
            },
            _ => Err(ZomeApiError::Internal(
                "get_links did not return an app entry".to_string(),
            )),
        });

    Ok(results_with_errors_too
        .filter_map(Result::ok)
        .collect())
}

///
/// Helper function for loading an entry and converting to a given type
///
pub fn get_entry_as_type_with_address<R: TryFrom<AppEntryValue> + Debug + Serialize + Clone>(address: Address) -> ZomeApiResult<EntryWithAddress<R>> {
    let get_result = hdk::get_entry(&address)?;
    let entry =
        get_result.ok_or_else(|| ZomeApiError::Internal("No entry at this address".into()))?;
    match entry.clone() {
        Entry::App(_, entry_value) => {
            let typed_entry = R::try_from(entry_value.to_owned()).map_err(|_| {
                ZomeApiError::Internal(
                    "Could not convert get_links result to requested type".to_string(),
                )
            })?;

            Ok(EntryWithAddress {
                entry: typed_entry,
                address: hdk::entry_address(&entry)?
            })
        },
        _ => Err(ZomeApiError::Internal(
            "get_links did not return an app entry".to_string(),
        )),
    }
}
