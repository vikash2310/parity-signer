//! All db handling related to seeds and addresses
//! seed phrases should be stored in hw encrypted by
//! best available tool and here they are only processed in plaintext.
//! Zeroization is mostly delegated to os

use bip39::{Language, Mnemonic, MnemonicType};
use lazy_static::lazy_static;
use parity_scale_codec::Encode;
use regex::Regex;
use sled::Batch;
use sp_core::{Pair, ed25519, sr25519, ecdsa};
use sp_runtime::MultiSigner;
use zeroize::Zeroize;

use constants::{ADDRTREE, TRANSACTION};
use defaults::get_default_chainspecs;
use definitions::{crypto::Encryption, error::{Active, AddressGeneration, AddressGenerationCommon, ErrorActive, ErrorSigner, ErrorSource, ExtraAddressGenerationSigner, InputActive, InputSigner, InterfaceSigner, Signer, SpecsKeySource}, helpers::multisigner_to_public, history::{Event, IdentityHistory}, keyring::{NetworkSpecsKey, AddressKey}, network_specs::NetworkSpecs, qr_transfers::ContentDerivations, users::AddressDetails};

use crate::db_transactions::{TrDbCold, TrDbColdDerivations};
use crate::helpers::{open_db, open_tree, make_batch_clear_tree, upd_id_batch, get_network_specs, get_address_details};
use crate::interface_signer::addresses_set_seed_name_network;
use crate::manage_history::{events_to_batch};
use crate::network_details::get_all_networks;

const ALICE_SEED_PHRASE: &str = "bottom drive obey lake curtain smoke basket hold race lonely fit walk";

lazy_static! {
// stolen from sp_core
// removed seed phrase part
// last '+' used to be '*', but empty password is an error
    static ref REG_PATH: Regex = Regex::new(r"^(?P<path>(//?[^/]+)*)(///(?P<password>.+))?$").expect("known value");
}

/// Get all identities from database.
/// Function gets used only on the Signer side.
pub fn get_all_addresses (database_name: &str) -> Result<Vec<(MultiSigner, AddressDetails)>, ErrorSigner> {
    let database = open_db::<Signer>(database_name)?;
    let identities = open_tree::<Signer>(&database, ADDRTREE)?;
    let mut out: Vec<(MultiSigner, AddressDetails)> = Vec::new();
    for (address_key_vec, address_entry) in identities.iter().flatten() {
        let address_key = AddressKey::from_ivec(&address_key_vec);
        let (multisigner, address_details) = AddressDetails::process_entry_with_key_checked::<Signer>(&address_key, address_entry)?;
        out.push((multisigner, address_details));
    }
    Ok(out)
}

/// Filter identities by given seed_name.
/// Function gets used only on the Signer side.
pub fn get_addresses_by_seed_name (database_name: &str, seed_name: &str) -> Result<Vec<(MultiSigner, AddressDetails)>, ErrorSigner> {
    Ok(get_all_addresses(database_name)?.into_iter().filter(|(_, address_details)| address_details.seed_name == seed_name).collect())
}

/// Generate random phrase with given number of words.
/// Function gets used only on the Signer side.
/// Open to user interface.
pub fn generate_random_phrase (words_number: u32) -> Result<String, ErrorSigner> {
    let mnemonic_type = match MnemonicType::for_word_count(words_number as usize) {
        Ok(a) => a,
        Err(e) => return Err(ErrorSigner::AddressGeneration(AddressGeneration::Extra(ExtraAddressGenerationSigner::RandomPhraseGeneration(e)))),
    };
    let mnemonic = Mnemonic::new(mnemonic_type, Language::English);
    Ok(mnemonic.into_phrase())
}

/// Create address from seed and path and insert it into the database.
/// Helper gets used both on Active side (when generating test cold database with well-known addresses)
/// and on Signer side (when real addresses are actually created by the user).
fn create_address<T: ErrorSource> (database_name: &str, input_batch_prep: &[(AddressKey, AddressDetails)], path: &str, network_specs: &NetworkSpecs, seed_name: &str, seed_phrase: &str) -> Result<(Vec<(AddressKey, AddressDetails)>, Vec<Event>), T::Error> {
    
    let mut output_batch_prep = input_batch_prep.to_vec();
    let mut output_events: Vec<Event> = Vec::new();
    let network_specs_key = NetworkSpecsKey::from_parts(&network_specs.genesis_hash.to_vec(), &network_specs.encryption);
    
    let mut full_address = seed_phrase.to_owned() + path;
    let multisigner = match network_specs.encryption {
        Encryption::Ed25519 => {
            match ed25519::Pair::from_string(&full_address, None) {
                Ok(a) => {
                    full_address.zeroize();
                    MultiSigner::Ed25519(a.public())
                },
                Err(e) => {
                    full_address.zeroize();
                    return Err(<T>::address_generation_common(AddressGenerationCommon::SecretString(e)))
                },
            }
        },
        Encryption::Sr25519 => {
            match sr25519::Pair::from_string(&full_address, None) {
                Ok(a) => {
                    full_address.zeroize();
                    MultiSigner::Sr25519(a.public())
                },
                Err(e) => {
                    full_address.zeroize();
                    return Err(<T>::address_generation_common(AddressGenerationCommon::SecretString(e)))
                },
            }
        },
        Encryption::Ecdsa => {
            match ecdsa::Pair::from_string(&full_address, None) {
                Ok(a) => {
                    full_address.zeroize();
                    MultiSigner::Ecdsa(a.public())
                },
                Err(e) => {
                    full_address.zeroize();
                    return Err(<T>::address_generation_common(AddressGenerationCommon::SecretString(e)))
                },
            }
        },
    };
    
    let public_key = multisigner_to_public(&multisigner);
    let address_key = AddressKey::from_multisigner(&multisigner);
    
    let (cropped_path, has_pwd) = match REG_PATH.captures(path) {
        Some(caps) => match caps.name("path") {
            Some(a) => (a.as_str(), caps.name("password").is_some()),
            None => ("", caps.name("password").is_some()),
        },
        None => ("", false),
    };
    
    let identity_history = IdentityHistory::get(seed_name, &network_specs.encryption, &public_key, cropped_path, &network_specs.genesis_hash.to_vec());
    output_events.push(Event::IdentityAdded(identity_history));
    
    let mut number_in_current = None;
    
// check if the same address key already participates in current database transaction
    for (i, (x_address_key, x_address_details)) in output_batch_prep.iter().enumerate() {
        if (x_address_key == &address_key) && !x_address_details.network_id.contains(&network_specs_key) {
            number_in_current = Some(i);
            break;
        }
    }
    match number_in_current {
        Some(i) => {
            let mut mod_entry = output_batch_prep.remove(i);
            mod_entry.1.network_id.push(network_specs_key);
            output_batch_prep.push(mod_entry);
            Ok((output_batch_prep, output_events))
        },
        None => {
            let database = open_db::<T>(database_name)?;
            let identities = open_tree::<T>(&database, ADDRTREE)?;
        // This address might be already created; maybe we just need to allow its use in another network?
            match identities.get(address_key.key()) {
                Ok(Some(address_entry)) => {
                    let mut address_details = AddressDetails::from_entry_with_key_checked::<T>(&address_key, address_entry)?;
                    if address_details.path != cropped_path {return Err(<T>::address_generation_common(AddressGenerationCommon::KeyCollision{seed_name: address_details.seed_name}))}
                    if !address_details.network_id.contains(&network_specs_key) {
                        address_details.network_id.push(network_specs_key);
                        output_batch_prep.push((address_key, address_details));
                        Ok((output_batch_prep, output_events))
                    }
                    else {
                        Err(<T>::address_generation_common(AddressGenerationCommon::DerivationExists(multisigner, address_details, network_specs_key)))
                    }
                },
                Ok(None) => {
                    let address_details = AddressDetails {
                        seed_name: seed_name.to_string(),
                        path: cropped_path.to_string(),
                        has_pwd,
                        network_id: vec![network_specs_key],
                        encryption: network_specs.encryption.to_owned(),
                    };
                    output_batch_prep.push((address_key, address_details));
                    Ok((output_batch_prep, output_events))
                },
                Err(e) => Err(<T>::db_internal(e)),
            }
        },
    }
}

/// Create addresses for all default paths in all default networks, and insert them in the database
fn populate_addresses<T: ErrorSource> (database_name: &str, entry_batch: Batch, seed_name: &str, seed_phrase: &str, roots: bool) -> Result<(Batch, Vec<Event>), T::Error> {
// TODO: check zeroize
    let mut identity_adds: Vec<(AddressKey, AddressDetails)> = Vec::new();
    let mut current_events: Vec<Event> = Vec::new();
    let specs_set = get_all_networks::<T>(database_name)?;
    for network_specs in specs_set.iter() {
        if roots {
            let (adds, events) = create_address::<T> (database_name, &identity_adds, "", network_specs, seed_name, seed_phrase)?;
            identity_adds = adds;
            current_events.extend_from_slice(&events);
        }
        if let Ok((adds, events)) = create_address::<T> (database_name, &identity_adds, &network_specs.path_id, network_specs, seed_name, seed_phrase) {
            identity_adds = adds;
            current_events.extend_from_slice(&events);
        }
    }
    Ok((upd_id_batch(entry_batch, identity_adds), current_events))
}

/// Generate new seed and populate all known networks with default accounts
pub fn try_create_seed (seed_name: &str, seed_phrase: &str, roots: bool, database_name: &str) -> Result<(), ErrorSigner> {
    let mut events: Vec<Event> = vec![Event::SeedCreated(seed_name.to_string())];
    let (id_batch, add_events) = populate_addresses::<Signer>(database_name, Batch::default(), seed_name, seed_phrase, roots)?;
    events.extend_from_slice(&add_events);
    TrDbCold::new()
        .set_addresses(id_batch) // add addresses just made in populate_addresses
        .set_history(events_to_batch::<Signer>(database_name, events)?) // add corresponding history
        .apply::<Signer>(database_name)
}

/// Function removes address by multisigner identifier and and network id
/// Function removes network_key from network_id vector for database record with address_key corresponding to given public key
pub fn remove_key(database_name: &str, multisigner: &MultiSigner, network_specs_key: &NetworkSpecsKey) -> Result<(), ErrorSigner> {
    remove_keys_set(database_name, &[multisigner.to_owned()], network_specs_key)
}

/// Function removes a set of addresses within one network by set of multisigner identifier and and network id
/// Function removes network_key from network_id vector for a defined set of database records
pub fn remove_keys_set(database_name: &str, multiselect: &[MultiSigner], network_specs_key: &NetworkSpecsKey) -> Result<(), ErrorSigner> {
    let mut id_batch = Batch::default();
    let mut events: Vec<Event> = Vec::new();
    let network_specs = get_network_specs(database_name, network_specs_key)?;
    for multisigner in multiselect.iter() {
        let public_key = multisigner_to_public(multisigner);
        let address_key = AddressKey::from_multisigner(multisigner);
        let mut address_details = get_address_details(database_name, &address_key)?;
        let identity_history = IdentityHistory::get(&address_details.seed_name, &network_specs.encryption, &public_key, &address_details.path, &network_specs.genesis_hash.to_vec());
        events.push(Event::IdentityRemoved(identity_history));
        address_details.network_id = address_details.network_id.into_iter().filter(|id| id != network_specs_key).collect();
        if address_details.network_id.is_empty() {id_batch.remove(address_key.key())}
        else {id_batch.insert(address_key.key(), address_details.encode())}
    }
    TrDbCold::new()
        .set_addresses(id_batch) // modify existing address entries
        .set_history(events_to_batch::<Signer>(database_name, events)?) // add corresponding history
        .apply::<Signer>(database_name)
}

/// Add a bunch of new derived addresses, N+1, N+2, etc.
pub fn create_increment_set(increment: u32, multisigner: &MultiSigner, network_specs_key: &NetworkSpecsKey, seed_phrase: &str, database_name: &str) -> Result<(), ErrorSigner> {
    let address_details = get_address_details(database_name, &AddressKey::from_multisigner(multisigner))?;
    let existing_identities = addresses_set_seed_name_network(database_name, &address_details.seed_name, network_specs_key)?;
    let mut last_index = 0;
    for (_, details) in existing_identities.iter() {
        if let Some(("", suffix)) = details.path.split_once(&address_details.path) {
            if let Some(could_be_number) = suffix.get(2..) {
                if let Ok(index) = could_be_number.parse::<u32>() {
                    last_index = std::cmp::max(index+1, last_index);
                }
            }
        }
    }
    let network_specs = get_network_specs(database_name, network_specs_key)?;
    let mut identity_adds: Vec<(AddressKey, AddressDetails)> = Vec::new();
    let mut current_events: Vec<Event> = Vec::new();
    for i in 0..increment {
        let path = address_details.path.to_string() + "//" + &(last_index+i).to_string();
        let (adds, events) = create_address::<Signer>(database_name, &identity_adds, &path, &network_specs, &address_details.seed_name, seed_phrase)?;
        identity_adds = adds;
        current_events.extend_from_slice(&events);
    }
    let id_batch = upd_id_batch(Batch::default(), identity_adds);
    TrDbCold::new()
        .set_addresses(id_batch) // add created address
        .set_history(events_to_batch::<Signer>(database_name, current_events)?) // add corresponding history
        .apply::<Signer>(database_name)
}

/// Check derivation format and determine whether there is a password
pub fn is_passworded(path: &str) -> Result<bool, ErrorSigner> {
    match REG_PATH.captures(path) {
        Some(caps) => Ok(caps.name("password").is_some()),
        None => Err(ErrorSigner::AddressGeneration(AddressGeneration::Extra(ExtraAddressGenerationSigner::InvalidDerivation))),
    }
}

/// Enum to describe derivation proposed
pub enum DerivationCheck {
    BadFormat, // bad format, still gets json back into UI
    Password, // passworded, no dynamic checking for collisions
    NoPassword(Option<(MultiSigner, AddressDetails)>), // no password and optional collision
}

/// Function to check if proposed address collides,
/// i.e. if the proposed path for given seed name and network specs key already exists
pub fn derivation_check (seed_name: &str, path: &str, network_specs_key: &NetworkSpecsKey, database_name: &str) -> Result<DerivationCheck, ErrorSigner> {
    match is_passworded(path) {
        Ok(true) => Ok(DerivationCheck::Password),
        Ok(false) => {
            let mut found_collision = None;
            for (multisigner, address_details) in get_all_addresses(database_name)?.into_iter() {
                if (address_details.seed_name == seed_name)&&(address_details.path == path)&&(address_details.network_id.contains(network_specs_key))&&(!address_details.has_pwd) {
                    found_collision = Some((multisigner, address_details));
                    break;
                }
            }
            Ok(DerivationCheck::NoPassword(found_collision))
        },
        Err(_) => Ok(DerivationCheck::BadFormat)
    }
}

/// Function to cut the password to be verified later in UI.
/// Expects password, if sees no password, returns error.
pub fn cut_path(path: &str) -> Result<(String, String), ErrorSigner> {
    match REG_PATH.captures(path) {
        Some(caps) => {
            let cropped_path = match caps.name("path") {
                Some(a) => a.as_str().to_string(),
                None => "".to_string(),
            };
            match caps.name("password") {
                Some(pwd) => Ok((cropped_path, pwd.as_str().to_string())),
                None => Err(ErrorSigner::Interface(InterfaceSigner::LostPwd)),
            }
        },
        None => Err(ErrorSigner::AddressGeneration(AddressGeneration::Extra(ExtraAddressGenerationSigner::InvalidDerivation))),
    }
}

/// Generate new identity (api for create_address())
/// Function is open to user interface
pub fn try_create_address (seed_name: &str, seed_phrase: &str, path: &str, network_specs_key: &NetworkSpecsKey, database_name: &str) -> Result<(), ErrorSigner> {
    match derivation_check(seed_name, path, network_specs_key, database_name)? {
        DerivationCheck::BadFormat => Err(ErrorSigner::AddressGeneration(AddressGeneration::Extra(ExtraAddressGenerationSigner::InvalidDerivation))),
        DerivationCheck::NoPassword(Some((multisigner, address_details))) => Err(<Signer>::address_generation_common(AddressGenerationCommon::DerivationExists(multisigner, address_details, network_specs_key.to_owned()))),
        _ => {
            let network_specs = get_network_specs(database_name, network_specs_key)?;
            let (adds, events) = create_address::<Signer>(database_name, &Vec::new(), path, &network_specs, seed_name, seed_phrase)?;
            let id_batch = upd_id_batch(Batch::default(), adds);
            TrDbCold::new()
                .set_addresses(id_batch) // add created address
                .set_history(events_to_batch::<Signer>(database_name, events)?) // add corresponding history
                .apply::<Signer>(database_name)
        },
    }
}

/// Function to generate identities batch with Alice information
pub fn generate_test_identities (database_name: &str) -> Result<(), ErrorActive> {
    let (id_batch, events) = {
        let entry_batch = make_batch_clear_tree::<Active>(database_name, ADDRTREE)?;
        let mut events = vec![Event::IdentitiesWiped];
        let (mut id_batch, new_events) = populate_addresses::<Active>(database_name, entry_batch, "Alice", ALICE_SEED_PHRASE, true)?;
        events.extend_from_slice(&new_events);
        for network_specs in get_default_chainspecs().iter() {
            if (network_specs.name == "westend")&&(network_specs.encryption == Encryption::Sr25519) {
                let (adds, updated_events) = create_address::<Active>(database_name, &Vec::new(), "//Alice", network_specs, "Alice", ALICE_SEED_PHRASE)?;
                id_batch = upd_id_batch(id_batch, adds);
                events.extend_from_slice(&updated_events);
            }
        }
        (id_batch, events)
    };
    TrDbCold::new()
        .set_addresses(id_batch) // add created address
        .set_history(events_to_batch::<Active>(database_name, events)?) // add corresponding history
        .apply::<Active>(database_name)
}

/// Function to remove all identities associated with given seen_name
/// Function is open to the user interface
pub fn remove_seed (database_name: &str, seed_name: &str) -> Result<(), ErrorSigner> {
    let mut identity_batch = Batch::default();
    let mut events: Vec<Event> = Vec::new();
    let id_set = get_addresses_by_seed_name(database_name, seed_name)?;
    for (multisigner, address_details) in id_set.iter() {
        let address_key = AddressKey::from_multisigner(multisigner);
        identity_batch.remove(address_key.key());
        let public_key = multisigner_to_public(multisigner);
        for network_specs_key in address_details.network_id.iter() {
            let (genesis_hash_vec, _) = network_specs_key.genesis_hash_encryption::<Signer>(SpecsKeySource::AddrTree(address_key.to_owned()))?;
            let identity_history = IdentityHistory::get(seed_name, &address_details.encryption, &public_key, &address_details.path, &genesis_hash_vec);
            events.push(Event::IdentityRemoved(identity_history));
        }
    }
    TrDbCold::new()
        .set_addresses(identity_batch) // modify addresses
        .set_history(events_to_batch::<Signer>(database_name, events)?) // add corresponding history
        .apply::<Signer>(database_name)
}

/// Function to import derivations without password into Signer from qr code
/// i.e. create new (address key + address details) entry,
/// or add network specs key to the existing one
pub fn import_derivations (checksum: u32, seed_name: &str, seed_phrase: &str, database_name: &str) -> Result<(), ErrorSigner> {
    let content_derivations = TrDbColdDerivations::from_storage(database_name, checksum)?;
    let network_specs = content_derivations.network_specs();
    let mut adds: Vec<(AddressKey, AddressDetails)> = Vec::new();
    let mut events: Vec<Event> = Vec::new();
    for path in content_derivations.checked_derivations().iter() {
        match create_address::<Signer>(database_name, &adds, path, network_specs, seed_name, seed_phrase) {
            Ok((mod_adds, mod_events)) => {
                adds = mod_adds;
                events.extend_from_slice(&mod_events);
            },
            Err(ErrorSigner::AddressGeneration(AddressGeneration::Common(AddressGenerationCommon::DerivationExists(_, _, _)))) => (),
            Err(e) => return Err(e),
        }
    }
    let identity_batch = upd_id_batch(Batch::default(), adds);
    let transaction_batch = make_batch_clear_tree::<Signer>(database_name, TRANSACTION)?;
    TrDbCold::new()
        .set_addresses(identity_batch) // modify addresses
        .set_history(events_to_batch::<Signer>(database_name, events)?) // add corresponding history
        .set_transaction(transaction_batch) // clear transaction tree
        .apply::<Signer>(database_name)
}

/// Function to check derivations before offering user to import them
pub fn check_derivation_set(derivations: &[String]) -> Result<(), ErrorSigner> {
    for path in derivations.iter() {
        match REG_PATH.captures(path) {
            Some(caps) => if caps.name("password").is_some() {return Err(ErrorSigner::Input(InputSigner::OnlyNoPwdDerivations))},
            None => return Err(ErrorSigner::Input(InputSigner::InvalidDerivation(path.to_string()))),
        }
    }
    Ok(())
}

/// Function to prepare derivations export using regex and a text provided by the user
pub fn prepare_derivations_export (encryption: &Encryption, genesis_hash: &[u8;32], content: &str) -> Result<ContentDerivations, ErrorActive> {
    let mut derivations: Vec<String> = Vec::new();
    let content_set: Vec<&str> = content.trim().split('\n').collect();
    for path in content_set.iter() {
        if let Some(caps) = REG_PATH.captures(path) {
            if let Some(p) = caps.name("path") {
                if caps.name("password").is_none() {
                    derivations.push(p.as_str().to_string())
                }
            }
        }
    }
    if derivations.is_empty() {return Err(ErrorActive::Input(InputActive::NoValidDerivationsToExport))}
    else {
        println!("Found and used {} valid password-free derivations:", derivations.len());
        for x in derivations.iter() {
            println!("\"{}\"", x);
        }
    }
    Ok(ContentDerivations::generate(encryption, genesis_hash, &derivations))
}


#[cfg(test)]
mod tests {
    use sled::{Db, Tree, open, Batch};
    use std::fs;
    
    use constants::ADDRTREE;
    use defaults::get_default_chainspecs;
    use definitions::{crypto::Encryption, keyring::{AddressKey, NetworkSpecsKey}, network_specs::Verifier};
    
    use crate::{cold_default::{populate_cold_no_metadata, signer_init_with_cert, populate_cold_release}, db_transactions::TrDbCold, helpers::{open_db, open_tree, upd_id_batch}, interface_signer::addresses_set_seed_name_network, manage_history::print_history};
    
    use super::*;

    #[test]
    fn test_generate_random_seed_phrase() {
        let random_phrase = generate_random_phrase(24).unwrap();
        assert!(Mnemonic::validate(&random_phrase, Language::English).is_ok());
        assert!(generate_random_phrase(1).is_err());
        let random_phrase2 = generate_random_phrase(24).unwrap();
        assert!(Mnemonic::validate(&random_phrase2, Language::English).is_ok());
        assert!(random_phrase2 != random_phrase);
    }

    #[test]
    fn test_check_for_seed_validity() {
        assert!(Mnemonic::validate(ALICE_SEED_PHRASE, Language::English).is_ok());
        assert!(Mnemonic::validate("the fox is triangular", Language::English).is_err());
        assert!(Mnemonic::validate("", Language::English).is_err());
        assert!(Mnemonic::validate("низ ехать подчиняться озеро занавеска дым корзина держать гонка одинокий подходящий прогулка", Language::English).is_err());
    }

    #[test]
    fn test_generate_default_addresses_for_alice() {
        let dbname = "for_tests/test_generate_default_addresses_for_Alice";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        try_create_seed("Alice", ALICE_SEED_PHRASE, true, dbname).unwrap();
        {
            let database = open_db::<Signer>(dbname).unwrap();
            let addresses = open_tree::<Signer>(&database, ADDRTREE).unwrap();
            assert!(addresses.len() == 4, "real addresses length: {}", addresses.len());
        }
        let chainspecs = get_default_chainspecs();
        let default_addresses = addresses_set_seed_name_network (dbname, "Alice", &NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519)).unwrap();
        assert!(default_addresses.len()>0);
        assert!("[(MultiSigner::Sr25519(46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a (5DfhGyQd...)), AddressDetails { seed_name: \"Alice\", path: \"\", has_pwd: false, network_id: [NetworkSpecsKey([1, 128, 145, 177, 113, 187, 21, 142, 45, 56, 72, 250, 35, 169, 241, 194, 81, 130, 251, 142, 32, 49, 59, 44, 30, 180, 146, 25, 218, 122, 112, 206, 144, 195]), NetworkSpecsKey([1, 128, 176, 168, 212, 147, 40, 92, 45, 247, 50, 144, 223, 183, 230, 31, 135, 15, 23, 180, 24, 1, 25, 122, 20, 156, 169, 54, 84, 73, 158, 163, 218, 254]), NetworkSpecsKey([1, 128, 225, 67, 242, 56, 3, 172, 80, 232, 246, 248, 230, 38, 149, 209, 206, 158, 78, 29, 104, 170, 54, 193, 205, 44, 253, 21, 52, 2, 19, 243, 66, 62])], encryption: Sr25519 }), (MultiSigner::Sr25519(64a31235d4bf9b37cfed3afa8aa60754675f9c4915430454d365c05112784d05 (5ELf63sL...)), AddressDetails { seed_name: \"Alice\", path: \"//kusama\", has_pwd: false, network_id: [NetworkSpecsKey([1, 128, 176, 168, 212, 147, 40, 92, 45, 247, 50, 144, 223, 183, 230, 31, 135, 15, 23, 180, 24, 1, 25, 122, 20, 156, 169, 54, 84, 73, 158, 163, 218, 254])], encryption: Sr25519 })]" == format!("{:?}", default_addresses), "Default addresses:\n{:?}", default_addresses);
        let database: Db = open(dbname).unwrap();
        let identities: Tree = database.open_tree(ADDRTREE).unwrap();
        let test_key = AddressKey::from_parts(&hex::decode("46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a").unwrap(), &Encryption::Sr25519).unwrap();
        assert!(identities.contains_key(test_key.key()).unwrap());
        fs::remove_dir_all(dbname).unwrap();
    }

    #[test]
    fn must_check_for_valid_derivation_phrase() {
        assert!(!is_passworded("").expect("valid empty path"));
        assert!(is_passworded("//").is_err());
        assert!(!is_passworded("//path1").expect("valid path1"));
        assert!(!is_passworded("//path/path").expect("soft derivation"));
        assert!(!is_passworded("//path//path").expect("hard derivation"));
        assert!(is_passworded("//path///password").expect("path with password"));
        assert!(is_passworded("///").is_err());
        assert!(!is_passworded("//$~").expect("weird symbols"));
        assert!(is_passworded("abraca dabre").is_err());
        assert!(is_passworded("////").expect("//// - password is /"));
        assert!(is_passworded("//path///password///password").expect("password///password is a password"));
        assert!(!is_passworded("//путь").expect("valid utf8 abomination"));
    }

    #[test]
    fn test_derive() { 
        let dbname = "for_tests/test_derive";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        println!("[0]: {:?}, [1]: {:?}", chainspecs[0].name, chainspecs[1].name);
        let seed_name = "Alice";
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        let network_id_1 = NetworkSpecsKey::from_parts(&chainspecs[1].genesis_hash.to_vec(), &Encryption::Sr25519);
        let both_networks = vec![network_id_0.to_owned(), network_id_1.to_owned()];
        let only_one_network = vec![network_id_0.to_owned()];

        try_create_seed(seed_name, ALICE_SEED_PHRASE, true, dbname).unwrap();
        let (adds1, events1) = {
            create_address::<Signer>(dbname, &Vec::new(), "//Alice", &chainspecs[0], seed_name, ALICE_SEED_PHRASE).unwrap()
        };
        TrDbCold::new()
            .set_addresses(upd_id_batch(Batch::default(), adds1)) // modify addresses
            .set_history(events_to_batch::<Signer>(&dbname, events1).unwrap()) // add corresponding history
            .apply::<Signer>(&dbname).unwrap();
        let (adds2, events2) = {
            create_address::<Signer>(dbname, &Vec::new(), "//Alice", &chainspecs[1], seed_name, ALICE_SEED_PHRASE).unwrap()
        };
        TrDbCold::new()
            .set_addresses(upd_id_batch(Batch::default(), adds2)) // modify addresses
            .set_history(events_to_batch::<Signer>(&dbname, events2).unwrap()) // add corresponding history
            .apply::<Signer>(&dbname).unwrap();
        let (adds3, events3) = {
            create_address::<Signer>(dbname, &Vec::new(), "//Alice/1", &chainspecs[0], seed_name, ALICE_SEED_PHRASE).unwrap()
        };
        TrDbCold::new()
            .set_addresses(upd_id_batch(Batch::default(), adds3)) // modify addresses
            .set_history(events_to_batch::<Signer>(&dbname, events3).unwrap()) // add corresponding history
            .apply::<Signer>(&dbname).unwrap();
        let identities = get_addresses_by_seed_name (&dbname, seed_name).unwrap();
        println!("{:?}", identities);
        let mut flag0 = false;
        let mut flag1 = false;
        for (_, details) in identities {
            flag0 = flag0 || details.network_id == both_networks;
            flag1 = flag1 || details.network_id == only_one_network;
        }
        assert!(flag0, "Something is wrong with //Alice");
        assert!(flag1, "Something is wrong with //Alice/1");
        fs::remove_dir_all(dbname).unwrap();
    }

    #[test]
    fn test_identity_deletion() {
        let dbname = "for_tests/test_identity_deletion";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        try_create_seed("Alice", ALICE_SEED_PHRASE, true, dbname).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_specs_key_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        let network_specs_key_1 = NetworkSpecsKey::from_parts(&chainspecs[1].genesis_hash.to_vec(), &Encryption::Sr25519);
        let mut identities = addresses_set_seed_name_network (dbname, "Alice", &network_specs_key_0).expect("Alice should have some addresses by default");
        println!("{:?}", identities);
        let (key0, _) = identities.remove(0); //TODO: this should be root key
        let (key1, _) = identities.remove(0); //TODO: this should be network-specific key
        remove_key(dbname, &key0, &network_specs_key_0).expect("delete an address");
        remove_key(dbname, &key1, &network_specs_key_0).expect("delete another address");
        let identities = addresses_set_seed_name_network (dbname, "Alice", &network_specs_key_0).expect("Alice still should have some addresses after deletion of two");
        for (address_key, _) in identities {
            assert_ne!(address_key, key0);
            assert_ne!(address_key, key1);
        }
        let identities = addresses_set_seed_name_network (dbname, "Alice", &network_specs_key_1).expect("Alice still should have some addresses after deletion of two");
        let mut flag_to_check_key0_remains = false;
        for (address_key, _) in identities {
            if address_key == key0 {
                flag_to_check_key0_remains = true;
            }
            assert_ne!(address_key, key1);
        }
        assert!(flag_to_check_key0_remains, "An address that should have only lost network was removed entirely");
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn history_with_identities() {
        let dbname = "for_tests/history_with_identities";
        populate_cold_release(dbname).unwrap();
        signer_init_with_cert(dbname).unwrap();
        let history_printed = print_history(dbname).unwrap();
        let element1 = r#"{"event":"database_initiated"}"#;
        let element2 = r#"{"event":"general_verifier_added","payload":{"public_key":"c46a22b9da19540a77cbde23197e5fd90485c72b4ecf3c599ecca6998f39bd57","identicon":"89504e470d0a1a0a0000000d494844520000001e0000001e08060000003b30aea200000abd49444154789c7557097454e515fefeb7cd9b25934042820211886c9204900402c8264b50e400258280d0ca762278b0ca625b9082158ab21c7b400a85822c874221201e018f0610651142c3225b58840849484220c9ccbc376fef7d8350eb72cffc7366e6bdf77fffbdf7bbdfbdc31cc7c1af184fcba21533cbb23a1b86315837f4decc614de3fc711618105115de348d1a49928e89a2b84f1084c3eeedb45c136899b47e66bf06fce8015dd7476b9a36dd34cd1cdbb621891ea89a822f4e1420aaabe8dbf905344e4a85120d83e778f03c7f990eb1d6e3f1ac628c45690bd7019bd6ff01fd12700cd4b2ccac4844d9601a46bae390738cb98b3ef06cfad2116cdf91430ec780a75a36631be61f7092e2931dc3d0387a966ee420086295d7eb9d4687d819fbed813d02fb29700c943c9ca428ca5ac7b1c1091e9397028cbce5fd1e0ec7fef319c6cc7d0e097152ccc3f2bb2a164e5d8c292fbe85da88e57a6cdb866a598622d2f620f0e5b466d0beaeb9078801fe183806aaaaeabbb4e630bace09b2a987ef0815275741ad2d45938c91a88d4fc7d0d7db8323df445140f53d13eb16ec44afd466b8faf572889e20523a4f823f25830ea0d0ee8c27aff704028161b4bf6b31f087c031d068343a291c0eade5383a3585571064eefca681a8b9f00d780f258a329533b5101f9dfc1a7fdfb600167d1fdc2b0f0bc62fc0e9d559d06a55da06f02625a2637e1178b98103c732693351f6ca1f06fc81d7e8b29b73cb058e7d20c6666951bd4814241846d4967d715cd5cd1338b33a079c2851da04186185bc198decf15bf16dc931225404d91d06e0eac1e528d93d039ea08fb672a08754b41db5162dbb4f423414022f48a665db022fb009b22c6fa09b0417186e2eb5a8f1ed8ddb25e9ef6d9a6156d6dc16fa660f43fed01938b9221deadd4a085e06adde41c6cbcb6126bf81338709823c7eb2a38d260d0a716a652e1d90b674e845abc71be7b0ff4211b6ee5d0ed9e3b3a78d9ccf75cf1c10e525d69c4e50190326328d367473eb948583ccfdc78e080de280fb75c092592b31bc5d079cde3106b67a17c9996391dc633976ac8f43880ec1f3846033fc66820d56f13e4a0f2ea640f2683370296ec8691833bb0f044a6254075a366964ec78ef94d828b1f1528f479ac56c3a7654891eafa8be9d336c66a645f5ca8b14dafbf50a06767d8e88b30f5555d5d042113c96d618d72f5ad8bdc90faf8f4019094888a157ae821efd78549456c321d23569d614cb36cec1d2cd8b90d2d0079b9cab0da9cec6f97bd9339d726b98e0b46594dbcef5f5f5a73c928cc97fc9c5bea347d02008dc238f3f98b518fdd3dfc2273b0d522e11cdd35474e91dc5ae4df1b857c52877c47ca2f7a8c9f528bdc6e1dca90011d0419ffe0e42d27e8c9cfd02046250cce3a6492878bfd80a7883bcc7eb99c2a874e64522e1059407abbcea06bf62c7bbb8597913fd3a0fc4b8dc19d8fecf38dcaf61f0c80ec2f50ccfe545d0b8a981630713601a40a79c305db3b07d5d3c240ff185424fef189f6fe2e8e50dd8fac516c479fdc81f360b1ddb74b723d130277be4bd8cbc3d40e17dd6b62ddb1748e2ec5019ea2a2f2139ad2bcaab646c5bc3439438f2cc811266c8c83630688486bbb78a619951243fd119c5c78338f089087f1c859f5e6ef807bda8e1e96c0e774abe811c97045f723b844235a4761c0920bbcbeaeaea4a4c436f2d78139c3bc5ebd9b54ff261690ee446a9e834612f0e1f4cc7b745241612f1467090f78a01a5623a6e9e5de3a61849a97dd13c7b3b0a3635c2fd6ac225e0c79a391836b606570a46a0e6f257244440d39e6fa0e5c0c5248b21925e0eacaeb6f6a2653bed1c4bb34fafeac0a9f72a21fa64446ba348edfd3bb418bc01678f86a0aa3cd2da79286c47716a4f6ff0543a8cea478b1848eff721e4c4a9b8541c761d46464e00e1cb7fc3857ffd1e9e7809b64949a6d2ebf8ea0957d1e02a1aab2560db61ed2cadd62e5e99c1996a3da9940f3ab1ba71561eda8ff9376e5eac83aa7078a29597d87d08c59fe692348a604c2060156d7a2ec1636d66e2bb4bb5040ba46524e0f6d70b71f5e3b9f0247849bc889c9a890e530e21ae5937587ad8a150d7fe10ea864ee981b9ecfafec5b1538b2442d9f9853877a51f8e7f0e503f40b08183e1e3ea517e7638caaf1e2260fa8d5a62fbfe85f86c772b945ea707c9b3361d807e83aee1ccba3e089595516828fc5943d06ee436f2963a253de892eb18955437ba680ba28f332b8aa0545d407ccb675067b7c1f6d5844825239058848934593d6df41a5885caebbb605b0a925b0cc1954badb17f07885c8440a756c2c00b630db46e710735173f87144886dcbc2f0cd3746108970b316a7f4b15559921f2a2a51951bee0f0565c2fbf86fe590390fd642e36ae9260ea1c91cb417d1d436f128bcc2e3a8abf4980a103995911dcbf6b62f7e678f8fce43199aa3a183bc9c23dfb04761e2a40bc3f0ea3fa8e4752428aade91a47ddea2b1210fd5962f681802fdef9c38af16cfd9e6df07ba96b58c0fa773621d5330e7b7751c152034b799c4a294fc1e7bb03b8799573d511710920a68770f24b0925e76512101b393d1992db16216f667752401bba093cd331031bdff9d2a286c8fb7cdeb7620d5e51d4f3d5f72ada0e7d33d326af398f24a3a656c1e01e83b1e6cf9fa2f4964924b2d1348da1ecba8a828d41c85e8742f6a066fb3e1f46561f01df5f132890165a3de9c1079be762f1470b91924892695ba80b6bcee677f6b39c8c7e2a2722e361937893c69c6563e67433ce5c29111b044977ab2dfc71d29ff0daf3f938ffe91498a15b48797a32a4275ec5f63522e92f4926e55d5580a1e38044f1636a120bc10411ad072cc391b20a4c9c978786f18046010b784573d792d3428ba6adb7481e715c0cd8b22dd9d29dd263e70a93df5e35d1ae0dd7705dd39fc5a2691b50b239173557ce525ba4f0534e9f9ebc15e5ea68141db4492c38b4ca34d1a9c359147f9845930471cb06a4a08caed32f60c59e1528285c4dc4949cd7c72cc44b03f2a902edf6d4842ebac0a42b24289a966719f60ed2522b1cb9cf354e69c5eacb4ee1d4aa2e103c347e303e360824771e852ee3b7a1ec7639a29a8eb4b4e6b87c6009d5ec6c02a41a24d3430a9e1ab50ea9dd26a2bcec1a4da65e233e902882b317f97cbe3974cb8341808c68028b9ac5325d37de1478d1302c43740bffdc3fba522dde842053c82254db13b7e078bd880f364ea310463166f06c8ceb968b132bbb92bbf4228fdd7b3bbe5a0cb9612bdad8d6a92d4aa49285f1c1f80184e31a7b084c348999130e873fd6b4e85092729393fc5cf87611f7fd97f311ad2f47e3f62320654ec090e96d10524c784480da34b62d3d8856d62d5c3dfc1e955d008f777b1d894f8d706c3d6c52971469c8bf120c063b5073a06480a3653f0476cd058f7da1da5e4983df34f72b2ff90dc678c13454161f0ce2ab93fb306ede60228d0cea34b853a360dee47731f5a5395409219a38248704c2b2b410510f8cfe5d14d28439e407d0586469e1c7c0ae3d02d775ed152ab3553420cb24707485593ce358448bb097dfeec5ce94dc20578086410e5b171e45ebd4743baa470994f18e4df3382f8006bb453453bb3975ed11a86b3f05768d5062e1a0c1d04e21d2cda4f50a7d4e748742afecc7ad3bdf61cbfe15442e05c3fafe1659ed7a51e8eb6211a014a9a2241510e85f69b8bf48fbb8e6ee47d9ff9ffd12f043237f1efc7fa27b92e83fd470d2f4a1a4745d45c193e4f7fa630f46358da95a244425729a06fcbd248705340e5da74baeb97b58b462f7fed8fe0bb74228b164fe5e050000000049454e44ae426082","encryption":"sr25519"}}"#;
        assert!(history_printed.contains(element1), "\nReal history check1:\n{}", history_printed);
        assert!(history_printed.contains(element2), "\nReal history check2:\n{}", history_printed);
        try_create_seed("Alice", ALICE_SEED_PHRASE, true, dbname).unwrap();
        let history_printed_after_create_seed = print_history(dbname).unwrap();
        let element3 = r#""events":[{"event":"seed_created","payload":"Alice"},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a","path":"","network_genesis_hash":"91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3"}},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"f606519cb8726753885cd4d0f518804a69a5e0badf36fee70feadd8044081730","path":"//polkadot","network_genesis_hash":"91b171bb158e2d3848fa23a9f1c25182fb8e20313b2c1eb49219da7a70ce90c3"}},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a","path":"","network_genesis_hash":"b0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe"}},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"64a31235d4bf9b37cfed3afa8aa60754675f9c4915430454d365c05112784d05","path":"//kusama","network_genesis_hash":"b0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe"}},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"46ebddef8cd9bb167dc30878d7113b7e168e6f0646beffd77d69d39bad76b47a","path":"","network_genesis_hash":"e143f23803ac50e8f6f8e62695d1ce9e4e1d68aa36c1cd2cfd15340213f3423e"}},{"event":"identity_added","payload":{"seed_name":"Alice","encryption":"sr25519","public_key":"3efeca331d646d8a2986374bb3bb8d6e9e3cfcdd7c45c2b69104fab5d61d3f34","path":"//westend","network_genesis_hash":"e143f23803ac50e8f6f8e62695d1ce9e4e1d68aa36c1cd2cfd15340213f3423e"}}]"#;
        assert!(history_printed_after_create_seed.contains(element1), "\nReal history check3:\n{}", history_printed_after_create_seed);
        assert!(history_printed_after_create_seed.contains(element2), "\nReal history check4:\n{}", history_printed_after_create_seed);
        assert!(history_printed_after_create_seed.contains(element3), "\nReal history check5:\n{}", history_printed_after_create_seed);
        fs::remove_dir_all(dbname).unwrap();
    }

    fn get_multisigner_path_set(dbname: &str) -> Vec<(MultiSigner, String)> {
        let db = open_db::<Signer>(dbname).unwrap();
        let identities = open_tree::<Signer>(&db, ADDRTREE).unwrap();
        let mut multisigner_path_set: Vec<(MultiSigner, String)> = Vec::new();
        for x in identities.iter() {
            if let Ok(a) = x {
                let (multisigner, address_details) = AddressDetails::process_entry_checked::<Signer>(a).unwrap();
                multisigner_path_set.push((multisigner, address_details.path.to_string()))
            }
        }
        multisigner_path_set
    }
    
    #[test]
    fn increment_identities_1() {
        let dbname = "for_tests/increment_identities_1";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        {
            let db = open_db::<Signer>(dbname).unwrap();
            let identities = open_tree::<Signer>(&db, ADDRTREE).unwrap();
            assert!(identities.len()==0);
        }
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        assert!(multisigner_path_set.len() == 1, "Wrong number of identities: {:?}", multisigner_path_set);
        println!("{}", multisigner_path_set[0].1);
        create_increment_set(4, &multisigner_path_set[0].0, &network_id_0, ALICE_SEED_PHRASE, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        assert!(multisigner_path_set.len() == 5, "Wrong number of identities after increment: {:?}", multisigner_path_set);
        let path_set: Vec<String> = multisigner_path_set.iter().map(|(_, path)| path.to_string()).collect();
        assert!(path_set.contains(&String::from("//Alice//0")));
        assert!(path_set.contains(&String::from("//Alice//1")));
        assert!(path_set.contains(&String::from("//Alice//2")));
        assert!(path_set.contains(&String::from("//Alice//3")));
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn increment_identities_2() {
        let dbname = "for_tests/increment_identities_2";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        {
            let db = open_db::<Signer>(dbname).unwrap();
            let identities = open_tree::<Signer>(&db, ADDRTREE).unwrap();
            assert!(identities.len()==0);
        }
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).unwrap();
        try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice//1", &network_id_0, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        let alice_multisigner_path = multisigner_path_set.iter().find(|(_, path)| path == "//Alice").unwrap();
        assert!(multisigner_path_set.len() == 2, "Wrong number of identities: {:?}", multisigner_path_set);
        create_increment_set(3, &alice_multisigner_path.0, &network_id_0, ALICE_SEED_PHRASE, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        assert!(multisigner_path_set.len() == 5, "Wrong number of identities after increment: {:?}", multisigner_path_set);
        let path_set: Vec<String> = multisigner_path_set.iter().map(|(_, path)| path.to_string()).collect();
        assert!(path_set.contains(&String::from("//Alice//2")));
        assert!(path_set.contains(&String::from("//Alice//3")));
        assert!(path_set.contains(&String::from("//Alice//4")));
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn increment_identities_3() {
        let dbname = "for_tests/increment_identities_3";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        {
            let db = open_db::<Signer>(dbname).unwrap();
            let identities = open_tree::<Signer>(&db, ADDRTREE).unwrap();
            assert!(identities.len()==0);
        }
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).unwrap();
        try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice//1", &network_id_0, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        let alice_multisigner_path = multisigner_path_set.iter().find(|(_, path)| path == "//Alice//1").unwrap();
        assert!(multisigner_path_set.len() == 2, "Wrong number of identities: {:?}", multisigner_path_set);
        create_increment_set(3, &alice_multisigner_path.0, &network_id_0, ALICE_SEED_PHRASE, dbname).unwrap();
        let multisigner_path_set = get_multisigner_path_set(dbname);
        assert!(multisigner_path_set.len() == 5, "Wrong number of identities after increment: {:?}", multisigner_path_set);
        let path_set: Vec<String> = multisigner_path_set.iter().map(|(_, path)| path.to_string()).collect();
        assert!(path_set.contains(&String::from("//Alice//1//0")));
        assert!(path_set.contains(&String::from("//Alice//1//1")));
        assert!(path_set.contains(&String::from("//Alice//1//2")));
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn checking_derivation_set() {
        assert!(check_derivation_set(&["/0".to_string(), "//Alice/westend".to_string(), "//secret//westend".to_string()]).is_ok());
        assert!(check_derivation_set(&["/0".to_string(), "/0".to_string(), "//Alice/westend".to_string(), "//secret//westend".to_string()]).is_ok());
        assert!(check_derivation_set(&["//remarkably///ugly".to_string()]).is_err());
        assert!(check_derivation_set(&["no_path_at_all".to_string()]).is_err());
        assert!(check_derivation_set(&["///".to_string()]).is_err());
    }
    
    #[test]
    fn creating_derivation_1() {
        let dbname = "for_tests/creating_derivation_1";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).is_ok(), "Should be able to create //Alice derivation.");
        if let DerivationCheck::NoPassword(Some(_)) = derivation_check("Alice", "//Alice", &network_id_0, dbname).unwrap() {println!("Found existing");}
        else {panic!("Derivation should already exist.");}
        match try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname) {
            Ok(()) => panic!("Should NOT be able to create //Alice derivation again."),
            Err(e) => assert!(<Signer>::show(&e) == "Error generating address. Seed Alice already has derivation //Alice for network specs key 0180b0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe, public key d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d.", "Wrong error: {}", <Signer>::show(&e)),
        }
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn creating_derivation_2() {
        let dbname = "for_tests/creating_derivation_2";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret", &network_id_0, dbname).is_ok(), "Should be able to create //Alice/// secret derivation.");
        if let DerivationCheck::NoPassword(None) = derivation_check("Alice", "//Alice", &network_id_0, dbname).unwrap() {println!("It did well.");}
        else {panic!("New derivation has no password, existing derivation has password and is diffenent.");}
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).is_ok(), "Should be able to create //Alice derivation.");
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn creating_derivation_3() {
        let dbname = "for_tests/creating_derivation_3";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice", &network_id_0, dbname).is_ok(), "Should be able to create //Alice derivation.");
        if let DerivationCheck::Password = derivation_check("Alice", "//Alice///secret", &network_id_0, dbname).unwrap() {println!("It did well.");}
        else {panic!("New derivation has password, existing derivation has no password and is diffenent.");}
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret", &network_id_0, dbname).is_ok(), "Should be able to create //Alice///secret derivation.");
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn creating_derivation_4() {
        let dbname = "for_tests/creating_derivation_4";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret1", &network_id_0, dbname).is_ok(), "Should be able to create //Alice///secret1 derivation.");
        if let DerivationCheck::Password = derivation_check("Alice", "//Alice///secret2", &network_id_0, dbname).unwrap() {println!("It did well.");}
        else {panic!("Existing derivation has different password.");}
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret2", &network_id_0, dbname).is_ok(), "Should be able to create //Alice///secret2 derivation.");
        fs::remove_dir_all(dbname).unwrap();
    }
    
    #[test]
    fn creating_derivation_5() {
        let dbname = "for_tests/creating_derivation_5";
        populate_cold_no_metadata(dbname, Verifier(None)).unwrap();
        let chainspecs = get_default_chainspecs();
        let network_id_0 = NetworkSpecsKey::from_parts(&chainspecs[0].genesis_hash.to_vec(), &Encryption::Sr25519);
        assert!(try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret", &network_id_0, dbname).is_ok(), "Should be able to create //Alice derivation.");
        if let DerivationCheck::Password = derivation_check("Alice", "//Alice///secret", &network_id_0, dbname).unwrap() {println!("It did well.");}
        else {panic!("Derivation exists, but has password.");}
        match try_create_address("Alice", ALICE_SEED_PHRASE, "//Alice///secret", &network_id_0, dbname) {
            Ok(()) => panic!("Should NOT be able to create //Alice///secret derivation again."),
            Err(e) => assert!(<Signer>::show(&e) == "Error generating address. Seed Alice already has derivation //Alice///<password> for network specs key 0180b0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe, public key 08a5e583f74f54f3811cb5f7d74e686d473e3a466fd0e95738707a80c3183b15.", "Wrong error: {}", <Signer>::show(&e)),
        }
        fs::remove_dir_all(dbname).unwrap();
    }
}

