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
        offer_definition()
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
        get_profile: {
            inputs: |profile_address: Address|,
            outputs: |result: ZomeApiResult<EntryWithAddress<Profile>>|,
            handler: handle_get_profile
        }
        find_profiles: {
            inputs: |nickname_prefix: String|,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Profile>>>|,
            handler: handle_find_profiles
        }

        create_offer: {
            inputs: |iam_offering: String, iam_requesting: Vec<String>|,
            outputs: |result: ZomeApiResult<Address>|,
            handler: handle_create_offer
        }
        get_my_offers: {
            inputs: | |,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Offer>>>|,
            handler: handle_get_my_offers
        }
        find_offers: {
            inputs: |i_want: String|,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Offer>>>|,
            handler: handle_find_offers
        }
        remove_offer: {
            inputs: |offer_address: Address|,
            outputs: |result: ZomeApiResult<()>|,
            handler: handle_remove_offer
        }
        find_deep_offers_i_want: {
            inputs: |i_want: String, max_swaps: i32|,
            outputs: |result: ZomeApiResult<Vec<Vec<EntryWithAddress<Offer>>>>|,
            handler: handle_find_deep_offers_i_want
        }
    ]
    traits: {
        hc_public [create_profile, get_my_profile, get_profile, find_profiles,
                   create_offer, get_my_offers, find_offers, remove_offer,
                   find_deep_offers_i_want]
    }
}

fn handle_get_profile(profile_address: Address) -> ZomeApiResult<EntryWithAddress<Profile>> {
    get_entry_as_type_with_address(profile_address)
}

fn handle_get_my_profile() -> ZomeApiResult<EntryWithAddress<Profile>> {
    let profile_address = get_my_profile_address()?;

    get_entry_as_type_with_address(profile_address)
}

fn handle_find_profiles(nickname_prefix: String) -> ZomeApiResult<Vec<EntryWithAddress<Profile>>> {
    let profiles_anchor_address = get_anchor_address("profiles")?;

    get_links_and_load_type_with_address::<Profile>(&profiles_anchor_address, LinkMatch::Exactly("anchor_profile"), LinkMatch::Regex(&format!("^{}.*", &regex::escape(&nickname_prefix))))
}

fn handle_remove_offer(offer_address: Address) -> ZomeApiResult<()> {
    let offer = get_entry_as_type_with_address::<Offer>(offer_address.clone())?.entry;

    let offered_item_address = get_or_create_item(offer.offering.clone())?;
    let profile_address = get_my_profile_address()?;
    let offers_anchor_address = get_anchor_address("offers")?;

    hdk::remove_link(&offers_anchor_address, &offer_address, "anchor_offer", "")?;
    hdk::remove_link(&offered_item_address, &offer_address, "item_offer", "offering")?;
    hdk::remove_link(&profile_address, &offer_address, "profile_offer", "")?;
    hdk::remove_entry(&offer_address)?;
    Ok(())
}

fn handle_find_offers(i_want: String) -> ZomeApiResult<Vec<EntryWithAddress<Offer>>> {
    if i_want == "" {
        let offers_anchor_address = get_anchor_address("offers")?;
        get_links_and_load_type_with_address(&offers_anchor_address, LinkMatch::Exactly("anchor_offer"), LinkMatch::Any)
    } else {
        let sought_item_address = get_or_create_item(i_want.clone())?;
        get_links_and_load_type_with_address(&sought_item_address, LinkMatch::Exactly("item_offer"), LinkMatch::Any)
    }
}

fn handle_get_my_offers() -> ZomeApiResult<Vec<EntryWithAddress<Offer>>> {
    let profile_address = get_my_profile_address()?;

    get_links_and_load_type_with_address(&profile_address, LinkMatch::Exactly("profile_offer"), LinkMatch::Any)
}

fn handle_find_deep_offers_i_want_0(i_want: String,
                                    max_swaps: i32,
                                    already_swapped: Vec<EntryWithAddress<Offer>>,
                                    results: &mut Vec<Vec<EntryWithAddress<Offer>>>) -> ZomeApiResult<()> {
    if max_swaps == 0 {
        Ok(())
    } else {
        handle_find_offers(i_want)?
            .iter()
            .map(|ex| {
                let entry = [ vec![ex.clone()].as_slice(), already_swapped.as_slice() ].concat();
                results.push(entry.clone());
                ex.entry.requesting
                    .iter()
                    .map(|offer_wants| {
                        handle_find_deep_offers_i_want_0(offer_wants.clone(), max_swaps - 1, entry.clone(), results)
                    })
                    .collect()
            })
            .collect()
    }
}

fn handle_find_deep_offers_i_want(i_want: String, max_swaps: i32) -> ZomeApiResult<Vec<Vec<EntryWithAddress<Offer>>>>{
    let mut result = Vec::new();
    handle_find_deep_offers_i_want_0(i_want, max_swaps, vec![], &mut result)?;
    Ok(result.clone())
}

fn handle_create_offer(iam_offering: String, iam_requesting: Vec<String>) -> ZomeApiResult<Address> {
    let offered_item_address = get_or_create_item(iam_offering.clone())?;
    let profile_address = get_my_profile_address()?;
    let offers_anchor_address = get_anchor_address("offers")?;

    let offer = Entry::App(
        "offer".into(),
        Offer {
            requesting: iam_requesting.clone(),
            offering: iam_offering.clone(),
            profile: profile_address.to_string().into()
        }.into()
    );

    let offer_address = hdk::commit_entry(&offer)?;

    hdk::link_entries(&offers_anchor_address, &offer_address, "anchor_offer", "")?;
    hdk::link_entries(&offered_item_address, &offer_address, "item_offer", "offering")?;
    hdk::link_entries(&profile_address, &offer_address, "profile_offer", "")?;

    Ok(offer_address)
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
                "offer",
                link_type: "anchor_offer",
                validation_package: || hdk::ValidationPackageDefinition::Entry,
                validation: |_validation_data: hdk::LinkValidationData| {
                    Ok(())
                }
            )
        ]
    )
}

fn get_anchor_address(name: &'static str) -> ZomeApiResult<Address> {
    let offers_anchor = Entry::App(
        "anchor".into(),
        RawString::from(name).into()
    );

    hdk::commit_entry(&offers_anchor)
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
                "offer",
                link_type: "profile_offer",
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
                "offer",
                link_type: "item_offer",
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

// ============================================================================ OFFER

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Offer {
    requesting: Vec<String>,
    offering: String,
    profile: Address
}

pub fn offer_definition() -> ValidatingEntryType {
    entry!(
        name: "offer",
        description: "",
        sharing: Sharing::Public,
        validation_package: || hdk::ValidationPackageDefinition::Entry,
        validation: |_validation_data: hdk::EntryValidationData<Offer>| {
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
