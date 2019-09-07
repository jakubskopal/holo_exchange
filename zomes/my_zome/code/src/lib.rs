#[macro_use]
extern crate hdk;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate holochain_json_derive;

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
    error::ZomeApiError,
};

use std::convert::TryFrom;
use serde::export::fmt::Debug;
use serde::Serialize;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntryWithAddress<T: Debug + Serialize> {
    address: Address,
    entry: T
}

impl <T: Debug + Serialize> From<EntryWithAddress<T>> for JsonString {
    fn from(v: EntryWithAddress<T>) -> Self {
        let s = &format!(r#"{{ "address": {}, "entry": {} }}"#,
                        serde_json::to_string(&v.address).unwrap_or_else(|_| panic!("could not Jsonify: {:?}", v)),
                        serde_json::to_string(&v.entry).unwrap_or_else(|_| panic!("could not Jsonify: {:?}", v))
        );
        JsonString::from_json(s)
    }
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

// ============================================================================ EXCHANGE

#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
pub struct Exchange {
    offering: String,
    requesting: String,
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
            outputs: |result: ZomeApiResult<()>|,
            handler: handle_create_profile
        }
        create_exchange: {
            inputs: |offering: String, requesting: String|,
            outputs: |result: ZomeApiResult<Address>|,
            handler: handle_create_exchange
        }
        find_exchanges: {
            inputs: |offering: String, requesting: String|,
            outputs: |result: ZomeApiResult<Vec<EntryWithAddress<Exchange>>>|,
            handler: handle_find_exchanges
        }
    ]
    traits: {
        hc_public [create_profile, create_exchange, find_exchanges]
    }
}

fn get_or_create_item(name: String) -> ZomeApiResult<Address> {
    let items_anchor = Entry::App(
        "anchor".into(),
        RawString::from("items").into()
    );

    let items_anchor_address = hdk::commit_entry(&items_anchor)?;

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

fn get_my_profile_address() -> ZomeApiResult<Address> {
    let address = hdk::get_links(&AGENT_ADDRESS, LinkMatch::Exactly("agent_profile"), LinkMatch::Any)?.addresses()
        .iter()
        .next()
        .ok_or(ZomeApiError::Internal("We do not seem to have a profile".into()))?
        .clone();

    Ok(address)
}

///
/// Helper function that perfoms a try_from for every entry
/// of a get_links_and_load for a given type. Any entries that either fail to
/// load or cannot be converted to the type will be dropped.
///
pub fn get_links_and_load_type_with_address<R: TryFrom<AppEntryValue> + Debug + Serialize>(
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

fn handle_find_exchanges(offering: String, requesting: String) -> ZomeApiResult<Vec<EntryWithAddress<Exchange>>> {
    // TODO: rewrite to use the anchors

    let exchanges_anchor = Entry::App(
        "anchor".into(),
        RawString::from("exchanges").into()
    );

    let exchanges_anchor_address = hdk::entry_address(&exchanges_anchor)?;

    let exchanges = get_links_and_load_type_with_address::<Exchange>(&exchanges_anchor_address, LinkMatch::Exactly("anchor_exchange"), LinkMatch::Any)?
        .iter()
        .filter(|&p| {
            (offering == "" || offering == p.entry.offering) && (requesting == "" || requesting == p.entry.requesting)
        })
        .map(|e| { e.clone() })
        .collect();

    Ok(exchanges)
}

fn handle_create_exchange(offering: String, requesting: String) -> ZomeApiResult<Address> {
    let offered_item = get_or_create_item(offering.clone())?;
    let requested_item = get_or_create_item(requesting.clone())?;
    let profile = get_my_profile_address()?;

    let exchanges_anchor = Entry::App(
        "anchor".into(),
        RawString::from("exchanges").into()
    );

    let exchanges_anchor_address = hdk::commit_entry(&exchanges_anchor)?;

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

fn handle_create_profile(nickname: String) -> ZomeApiResult<()> {
    let profiles_anchor = Entry::App(
        "anchor".into(),
        RawString::from("profiles").into()
    );

    let profiles_anchor_address = hdk::commit_entry(&profiles_anchor)?;

    let profile = Entry::App(
        "profile".into(),
        Profile {
            nickname: nickname.clone(),
            address: AGENT_ADDRESS.to_string().into()
        }.into()
    );

    let profile_address = hdk::commit_entry(&profile)?;

    hdk::link_entries(&profiles_anchor_address, &profile_address, "anchor_profile".into(), nickname.clone())?;

    hdk::link_entries(&AGENT_ADDRESS, &profile_address, "agent_profile".into(), "")?;

    Ok(())
}

//#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
//struct List {
//    name: String
//}
//
//#[derive(Serialize, Deserialize, Debug, Clone, DefaultJson)]
//struct ListItem {
//    text: String,
//    completed: bool
//}
//
//#[derive(Serialize, Deserialize, Debug, DefaultJson)]
//struct GetListResponse {
//    name: String,
//    items: Vec<ListItem>
//}
//
//fn handle_create_list(list: List) -> ZomeApiResult<Address> {
//    // define the entry
//    let list_entry = Entry::App(
//        "list".into(),
//        list.into()
//    );
//
//    // commit the entry and return the address
//    hdk::commit_entry(&list_entry)
//}
//
//
//fn handle_add_item(list_item: ListItem, list_addr: HashString) -> ZomeApiResult<Address> {
//    // define the entry
//    let list_item_entry = Entry::App(
//        "listItem".into(),
//        list_item.into()
//    );
//
//    let item_addr = hdk::commit_entry(&list_item_entry)?; // commit the list item
//    hdk::link_entries(&list_addr, &item_addr, "items", "")?; // if successful, link to list address
//    Ok(item_addr)
//}
//
//
//fn handle_get_list(list_addr: HashString) -> ZomeApiResult<GetListResponse> {
//
//    // load the list entry. Early return error if it cannot load or is wrong type
//    let list = hdk::utils::get_as_type::<List>(list_addr.clone())?;
//
//    // try and load the list items, filter out errors and collect in a vector
//    let list_items = hdk::get_links(&list_addr, LinkMatch::Exactly("items"), LinkMatch::Any)?.addresses()
//        .iter()
//        .map(|item_address| {
//            hdk::utils::get_as_type::<ListItem>(item_address.to_owned())
//        })
//        .filter_map(Result::ok)
//        .collect::<Vec<ListItem>>();
//
//    // if this was successful then return the list items
//    Ok(GetListResponse{
//        name: list.name,
//        items: list_items
//    })
//}
