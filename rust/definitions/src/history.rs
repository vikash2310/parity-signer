//! Types used in Signer history log
//!
//! Signer keeps log of all events in `HISTORY` tree of the cold database.
//!
//! Every log [`Entry`] consists of timestamp and a set of simultaneously
//! occured events `Vec<Event>`. [`Entry`] is stored SCALE-encoded under key
//! [`Order`](crate::keyring::Order). `Order` is produced from the order the
//! event gets in history log when entered. `Order` is an addition to the
//! timestamp, and normally arranging entries by the timestamp should
//! coincide with arranging entries by `Order`.  
//!
//! Log by default starts with database init `Entry` containing:
//!
//! - `Event::DatabaseInitiated`
//! - `Event::GeneralVerifierSet(_)`
//!
//! User can clear history log at any time. This indeed will remove all history
//! entries, and the log will then start with `Entry` containing
//! `Event::HistoryCleared`.
use blake2_rfc::blake2b::blake2b;
use parity_scale_codec::{Decode, Encode};
use sled::IVec;
#[cfg(feature = "signer")]
use std::convert::TryInto;

use crate::{
    crypto::{Encryption, MultiSigner},
    keyring::VerifierKey,
    metadata::MetaValues,
    network_specs::{
        NetworkSpecs, NetworkSpecsToSend, ValidCurrentVerifier, Verifier, VerifierValue,
    },
    qr_transfers::ContentLoadTypes,
};
#[cfg(feature = "signer")]
use crate::{
    helpers::{pic_meta, pic_types},
    print::{export_complex_single, export_complex_vector},
};

/// Event content for importing or removing metadata of a known network
///
/// Contains network name, network version, metadata hash.
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct MetaValuesDisplay {
    pub name: String,
    pub version: u32,
    pub meta_hash: Vec<u8>,
}

impl MetaValuesDisplay {
    /// Generate [`MetaValuesDisplay`] from [`MetaValues`]
    pub fn get(meta_values: &MetaValues) -> Self {
        Self {
            name: meta_values.name.to_string(),
            version: meta_values.version,
            meta_hash: blake2b(32, &[], &meta_values.meta).as_bytes().to_vec(),
        }
    }

    /// Generate [`MetaValuesDisplay`] from database entry with network name,
    /// network version, and stored metadata entry as is
    ///
    /// This is used for deletion, no checking of stored metadata integrity is
    /// made.
    pub fn from_storage(name: &str, version: u32, meta_stored: IVec) -> Self {
        Self {
            name: name.to_string(),
            version,
            meta_hash: blake2b(32, &[], &meta_stored).as_bytes().to_vec(),
        }
    }

    /// Print json with [`MetaValuesDisplay`] information for user interface  
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        let meta_id_pic = hex::encode(pic_meta(&self.meta_hash));
        format!("\"specname\":\"{}\",\"spec_version\":\"{}\",\"meta_hash\":\"{}\",\"meta_id_pic\":\"{}\"", &self.name, &self.version, hex::encode(&self.meta_hash), meta_id_pic)
    }
}

/// Event content for generating [`SufficientCrypto`](crate::crypto::SufficientCrypto)
/// QR code for `load_metadata` message  
///
/// Effectively records that network metadata was signed by user.
/// Contains network name, network version, metadata hash, and [`VerifierValue`]
/// of address used for `SufficientCrypto` generation.  
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct MetaValuesExport {
    pub name: String,
    pub version: u32,
    pub meta_hash: Vec<u8>,
    pub signed_by: VerifierValue,
}

impl MetaValuesExport {
    /// Generate [`MetaValuesExport`] from [`MetaValues`] and [`VerifierValue`]
    /// of address used for `SufficientCrypto` generation.  
    pub fn get(meta_values: &MetaValues, signed_by: &VerifierValue) -> Self {
        Self {
            name: meta_values.name.to_string(),
            version: meta_values.version,
            meta_hash: blake2b(32, &[], &meta_values.meta).as_bytes().to_vec(),
            signed_by: signed_by.to_owned(),
        }
    }

    /// Print json with [`MetaValuesExport`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        let meta_id_pic = hex::encode(pic_meta(&self.meta_hash));
        format!("\"specname\":\"{}\",\"spec_version\":\"{}\",\"meta_hash\":\"{}\",\"meta_id_pic\":\"{}\",\"signed_by\":{}", &self.name, &self.version, hex::encode(&self.meta_hash), meta_id_pic, export_complex_single(&self.signed_by, |a| a.show_card()))
    }
}

/// Event content for importing or removing network specs  
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct NetworkSpecsDisplay {
    pub specs: NetworkSpecs,
    pub valid_current_verifier: ValidCurrentVerifier,
    pub general_verifier: Verifier,
}

impl NetworkSpecsDisplay {
    /// Generate [`NetworkSpecsDisplay`] from [`NetworkSpecs`],
    /// network-associated [`ValidCurrentVerifier`], and
    /// general verifier [`Verifier`]
    pub fn get(
        specs: &NetworkSpecs,
        valid_current_verifier: &ValidCurrentVerifier,
        general_verifier: &Verifier,
    ) -> Self {
        Self {
            specs: specs.to_owned(),
            valid_current_verifier: valid_current_verifier.to_owned(),
            general_verifier: general_verifier.to_owned(),
        }
    }

    /// Print json with [`NetworkSpecsDisplay`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        self.specs
            .show(&self.valid_current_verifier, &self.general_verifier)
    }
}

/// Event content for generating [`SufficientCrypto`](crate::crypto::SufficientCrypto)
/// QR code for `add_specs` message  
///
/// Effectively records that network specs were signed by user.
/// Contains [`NetworkSpecsToSend`] and [`VerifierValue`] of address used for
/// `SufficientCrypto` generation.  
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct NetworkSpecsExport {
    pub specs_to_send: NetworkSpecsToSend,
    pub signed_by: VerifierValue,
}

impl NetworkSpecsExport {
    /// Generate [`NetworkSpecsExport`] from [`NetworkSpecsToSend`] and
    /// [`VerifierValue`] of address used for `SufficientCrypto` generation.  
    pub fn get(specs_to_send: &NetworkSpecsToSend, signed_by: &VerifierValue) -> Self {
        Self {
            specs_to_send: specs_to_send.to_owned(),
            signed_by: signed_by.to_owned(),
        }
    }

    /// Print json with [`NetworkSpecsExport`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        format!(
            "{},\"signed_by\":{}",
            &self.specs_to_send.show(),
            export_complex_single(&self.signed_by, |a| a.show_card())
        )
    }
}

/// Event content for setting network verifier
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct NetworkVerifierDisplay {
    pub genesis_hash: Vec<u8>,
    pub valid_current_verifier: ValidCurrentVerifier,
    pub general_verifier: Verifier,
}

impl NetworkVerifierDisplay {
    /// Generate [`NetworkVerifierDisplay`] from [`VerifierKey`],
    /// [`ValidCurrentVerifier`], the setting of which the event records,
    /// and general verifier [`Verifier`]  
    pub fn get(
        verifier_key: &VerifierKey,
        valid_current_verifier: &ValidCurrentVerifier,
        general_verifier: &Verifier,
    ) -> Self {
        Self {
            genesis_hash: verifier_key.genesis_hash(),
            valid_current_verifier: valid_current_verifier.to_owned(),
            general_verifier: general_verifier.to_owned(),
        }
    }

    /// Print json with [`NetworkVerifierDisplay`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        format!(
            "\"genesis_hash\":\"{}\",\"current_verifier\":{}",
            hex::encode(&self.genesis_hash),
            export_complex_single(&self.valid_current_verifier, |a| a
                .show(&self.general_verifier))
        )
    }
}

/// Event content for importing or removing types information
///
/// Contains hash of SCALE-encoded types data and types information [`Verifier`].
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct TypesDisplay {
    pub types_hash: Vec<u8>,
    pub verifier: Verifier,
}

impl TypesDisplay {
    /// Generate [`TypesDisplay`] from [`ContentLoadTypes`] and types information
    /// [`Verifier`]  
    pub fn get(types_content: &ContentLoadTypes, verifier: &Verifier) -> Self {
        Self {
            types_hash: blake2b(32, &[], &types_content.store()).as_bytes().to_vec(),
            verifier: verifier.to_owned(),
        }
    }

    /// Print json with [`TypesDisplay`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        let types_id_pic = hex::encode(pic_types(&self.types_hash));
        format!(
            "\"types_hash\":\"{}\",\"types_id_pic\":\"{}\",\"verifier\":{}",
            hex::encode(&self.types_hash),
            types_id_pic,
            export_complex_single(&self.verifier, |a| a.show_card())
        )
    }
}

/// Event content for generating [`SufficientCrypto`](crate::crypto::SufficientCrypto)
/// QR code for `load_types` message  
///
/// Effectively records that types information was signed by user.
/// Contains hash of SCALE-encoded types data and [`VerifierValue`] of address
/// used for `SufficientCrypto` generation.  
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct TypesExport {
    pub types_hash: Vec<u8>,
    pub signed_by: VerifierValue,
}

impl TypesExport {
    /// Generate [`TypesExport`] from [`ContentLoadTypes`] and [`VerifierValue`]
    /// of address used for `SufficientCrypto` generation  
    pub fn get(types_content: &ContentLoadTypes, signed_by: &VerifierValue) -> Self {
        Self {
            types_hash: blake2b(32, &[], &types_content.store()).as_bytes().to_vec(),
            signed_by: signed_by.to_owned(),
        }
    }

    /// Print json with [`TypesExport`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        let types_id_pic = hex::encode(pic_types(&self.types_hash));
        format!(
            "\"types_hash\":\"{}\",\"types_id_pic\":\"{}\",\"signed_by\":{}",
            hex::encode(&self.types_hash),
            types_id_pic,
            export_complex_single(&self.signed_by, |a| a.show_card())
        )
    }
}

/// Event content for address generation or removal
///
/// Contains public information associated with address:
/// - seed name  
/// - [`Encryption`]  
/// - public key for address  
/// - path with soft (`/`) and hard (`//`) derivatinos only, **without** password  
/// - genesis hash of the network within which the address is  
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct IdentityHistory {
    pub seed_name: String,
    pub encryption: Encryption,
    pub public_key: Vec<u8>,
    pub path: String,
    pub network_genesis_hash: Vec<u8>,
}

impl IdentityHistory {
    /// Generate [`IdentityHistory`] from parts  
    pub fn get(
        seed_name: &str,
        encryption: &Encryption,
        public_key: &[u8],
        path: &str,
        network_genesis_hash: &[u8],
    ) -> Self {
        Self {
            seed_name: seed_name.to_string(),
            encryption: encryption.to_owned(),
            public_key: public_key.to_vec(),
            path: path.to_string(),
            network_genesis_hash: network_genesis_hash.to_vec(),
        }
    }

    /// Print json with [`IdentityHistory`] information for user interface
    #[cfg(feature = "signer")]
    pub fn show(&self) -> String {
        format!("\"seed_name\":\"{}\",\"encryption\":\"{}\",\"public_key\":\"{}\",\"path\":\"{}\",\"network_genesis_hash\":\"{}\"", &self.seed_name, &self.encryption.show(), hex::encode(&self.public_key), &self.path, hex::encode(&self.network_genesis_hash))
    }
}

/// History log information about transactions, both successfully signed and
/// the ones with wrong password entered by user
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct SignDisplay {
    /// raw `Vec<u8>` transaction that user either tried to sign or signed  
    pub transaction: Vec<u8>,

    /// name for the network in which transaction is generated,
    /// as it is recorded in the network specs and network metadata  
    pub network_name: String,

    /// address that has generated and signed the transaction  
    pub signed_by: VerifierValue,

    /// user entered comment for transaction
    pub user_comment: String,
}

impl SignDisplay {
    /// Generate [`SignDisplay`] from parts  
    pub fn get(
        transaction: &[u8],
        network_name: &str,
        signed_by: &VerifierValue,
        user_comment: &str,
    ) -> Self {
        Self {
            transaction: transaction.to_vec(),
            network_name: network_name.to_string(),
            signed_by: signed_by.to_owned(),
            user_comment: user_comment.to_string(),
        }
    }

    /// Get raw transaction, network name, and [`Encryption`] from [`SignDisplay`]  
    pub fn transaction_network_encryption(&self) -> (Vec<u8>, String, Encryption) {
        let encryption = match &self.signed_by {
            VerifierValue::Standard {
                multi_signer: MultiSigner::Ed25519 { .. },
            } => Encryption::Ed25519,
            VerifierValue::Standard {
                multi_signer: MultiSigner::Sr25519 { .. },
            } => Encryption::Sr25519,
            VerifierValue::Standard {
                multi_signer: MultiSigner::Ecdsa { .. },
            } => Encryption::Ecdsa,
        };
        (
            self.transaction.to_vec(),
            self.network_name.to_string(),
            encryption,
        )
    }

    /// Get raw transaction from [`SignDisplay`]  
    pub fn transaction(&self) -> Vec<u8> {
        self.transaction.to_vec()
    }

    /// Print json with [`SignDisplay`] information in case of **successful**
    /// signing, for user interface
    ///
    /// Function to display transaction could vary,
    /// currently general log view shows raw hexadecimal transaction,
    /// detailed log view shows parsed transaction if parsing is possible.
    #[cfg(feature = "signer")]
    pub fn success<O>(&self, op: O) -> String
    where
        O: Fn(&Self) -> String + Copy,
    {
        format!(
            "\"transaction\":{},\"network_name\":\"{}\",\"signed_by\":{},\"user_comment\":\"{}\"",
            op(self),
            &self.network_name,
            export_complex_single(&self.signed_by, |a| a.show_card()),
            &self.user_comment
        )
    }

    /// Print json with [`SignDisplay`] information in case of **failed** signing,
    /// for user interface
    ///
    /// This is reserved for cases that have failed because user has entered
    /// a wrong password.
    ///
    /// Function to display transaction could vary,
    /// currently general log view shows raw hexadecimal transaction,
    /// detailed log view shows parsed transaction if parsing is possible.
    #[cfg(feature = "signer")]
    pub fn pwd_failure<O>(&self, op: O) -> String
    where
        O: Fn(&Self) -> String + Copy,
    {
        format!("\"transaction\":{},\"network_name\":\"{}\",\"signed_by\":{},\"user_comment\":\"{}\",\"error\":\"wrong_password_entered\"", op(self), &self.network_name, export_complex_single(&self.signed_by, |a| a.show_card()), &self.user_comment)
    }
}

/// History log information about messages, both successfully signed and
/// the ones with wrong password entered by user
#[derive(Decode, Encode, PartialEq, Clone)]
pub struct SignMessageDisplay {
    /// decoded message
    pub message: String,

    /// name for the network in which message transaction is generated,
    /// as it is recorded in the network specs and network metadata  
    pub network_name: String,

    /// address that has generated and signed the message  
    pub signed_by: VerifierValue,

    /// user entered comment for message
    pub user_comment: String,
}

impl SignMessageDisplay {
    /// Generate [`SignMessageDisplay`] from parts  
    pub fn get(
        message: &str,
        network_name: &str,
        signed_by: &VerifierValue,
        user_comment: &str,
    ) -> Self {
        Self {
            message: message.to_string(),
            network_name: network_name.to_string(),
            signed_by: signed_by.to_owned(),
            user_comment: user_comment.to_string(),
        }
    }

    /// Print json with [`SignMessageDisplay`] information in case of **successful**
    /// signing, for user interface
    #[cfg(feature = "signer")]
    pub fn success(&self) -> String {
        format!(
            "\"message\":\"{}\",\"network_name\":\"{}\",\"signed_by\":{},\"user_comment\":\"{}\"",
            hex::encode(&self.message.as_bytes()),
            &self.network_name,
            export_complex_single(&self.signed_by, |a| a.show_card()),
            &self.user_comment
        )
    }

    /// Print json with [`SignMessageDisplay`] information in case of **failed** signing,
    /// for user interface
    #[cfg(feature = "signer")]
    pub fn pwd_failure(&self) -> String {
        format!("\"message\":\"{}\",\"network_name\":\"{}\",\"signed_by\":{},\"user_comment\":\"{}\",\"error\":\"wrong_password_entered\"", hex::encode(&self.message.as_bytes()), &self.network_name, export_complex_single(&self.signed_by, |a| a.show_card()), &self.user_comment)
    }
}

/// Events that could be recorded in the history log
#[derive(Decode, Encode, Clone)]
pub enum Event {
    /// Network metadata was added
    MetadataAdded {
        meta_values_display: MetaValuesDisplay,
    },

    /// Network metadata was removed
    MetadataRemoved {
        meta_values_display: MetaValuesDisplay,
    },

    /// User has generated [`SufficientCrypto`](crate::crypto::SufficientCrypto)
    /// with one of Signer addresses for `load_metadata` update
    MetadataSigned {
        meta_values_export: MetaValuesExport,
    },

    /// Network specs were added
    NetworkSpecsAdded {
        network_specs_display: NetworkSpecsDisplay,
    },

    /// Network specs were removed
    NetworkSpecsRemoved {
        network_specs_display: NetworkSpecsDisplay,
    },

    /// User has generated [`SufficientCrypto`](crate::crypto::SufficientCrypto)
    /// with one of Signer addresses for `add_specs` update
    NetworkSpecsSigned {
        network_specs_export: NetworkSpecsExport,
    },

    /// Network verifier with [`ValidCurrentVerifier`] was set for network
    NetworkVerifierSet {
        network_verifier_display: NetworkVerifierDisplay,
    },

    /// General verifier was set up
    GeneralVerifierSet { verifier: Verifier },

    /// Types information was added
    TypesAdded { types_display: TypesDisplay },

    /// Types information was removed
    TypesRemoved { types_display: TypesDisplay },

    /// User has generated [`SufficientCrypto`](crate::crypto::SufficientCrypto)
    /// with one of Signer addresses for `load_types` update
    TypesSigned { types_export: TypesExport },

    /// User has generated signature for a transaction
    TransactionSigned { sign_display: SignDisplay },

    /// User tried to generate signature for a transaction, but failed to enter
    /// a valid password
    TransactionSignError { sign_display: SignDisplay },

    /// User has generated signature for a message
    MessageSigned {
        sign_message_display: SignMessageDisplay,
    },

    /// User tried to generate signature for a message, but failed to enter
    /// a valid password
    MessageSignError {
        sign_message_display: SignMessageDisplay,
    },

    /// User generated a new address
    IdentityAdded { identity_history: IdentityHistory },

    /// User removed an address
    IdentityRemoved { identity_history: IdentityHistory },

    /// All identities were wiped
    IdentitiesWiped,

    /// Signer was online, i.e. the air-gap was broken
    DeviceWasOnline,

    /// User has acknowledged the dangers detected and has reset the Signer
    /// danger status
    ResetDangerRecord,

    /// New seed was created (stored value here is the seed name)
    SeedCreated { seed_created: String },

    /// User opened seed backup, and seed phrase for shortly shown as a plain
    /// text on screen (stored value here is the seed name)
    SeedNameWasShown { seed_name_was_shown: String }, // for individual seed_name

    /// A warning was produces and displayed to user
    Warning { warning: String },

    /// User has entered wrong password
    WrongPassword,

    /// User has manually added entry to history log
    UserEntry { user_entry: String },

    /// System-generated entry into history log
    SystemEntry { system_entry: String },

    /// History was cleared
    HistoryCleared,

    /// Database was initiated
    DatabaseInitiated,
}

/// History log individual entry
///
/// Contains timestamp and set of simultaneously occured events `Vec<Event>`.
///
/// `Entry` is stored SCALE-encoded in the `HISTORY` tree of the cold database,
/// under key `Order`.
#[derive(Decode, Encode, Clone)]
pub struct Entry {
    pub timestamp: String,
    pub events: Vec<Event>, // events already in showable form
}

#[cfg(feature = "signer")]
impl Event {
    /// Print json with [`Event`] related information for user interface
    ///
    /// Required input is the function to print `SignDisplay` contents.
    ///
    /// Currently general log view shows raw hexadecimal transaction,
    /// detailed log view shows parsed transaction if parsing is possible.
    pub fn show<O>(&self, op: O) -> String
    where
        O: Fn(&SignDisplay) -> String + Copy,
    {
        match &self {
            Event::MetadataAdded {
                meta_values_display: x,
            } => format!(
                "\"event\":\"metadata_added\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::MetadataRemoved {
                meta_values_display: x,
            } => format!(
                "\"event\":\"metadata_removed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::MetadataSigned {
                meta_values_export: x,
            } => format!(
                "\"event\":\"load_metadata_message_signed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::NetworkSpecsAdded {
                network_specs_display: x,
            } => format!(
                "\"event\":\"network_specs_added\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::NetworkSpecsRemoved {
                network_specs_display: x,
            } => format!(
                "\"event\":\"network_removed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::NetworkSpecsSigned {
                network_specs_export: x,
            } => format!(
                "\"event\":\"add_specs_message_signed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::NetworkVerifierSet {
                network_verifier_display: x,
            } => format!(
                "\"event\":\"network_verifier_set\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::GeneralVerifierSet { verifier: x } => format!(
                "\"event\":\"general_verifier_added\",\"payload\":{}",
                export_complex_single(x, |a| a.show_card())
            ),
            Event::TypesAdded { types_display: x } => format!(
                "\"event\":\"types_added\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::TypesRemoved { types_display: x } => format!(
                "\"event\":\"types_removed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::TypesSigned { types_export: x } => format!(
                "\"event\":\"load_types_message_signed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::TransactionSigned { sign_display: x } => format!(
                "\"event\":\"transaction_signed\",\"payload\":{}",
                export_complex_single(x, |a| a.success(|b| op(b)))
            ),
            Event::TransactionSignError { sign_display: x } => format!(
                "\"event\":\"transaction_sign_error\",\"payload\":{}",
                export_complex_single(x, |a| a.pwd_failure(|b| op(b)))
            ),
            Event::MessageSigned {
                sign_message_display: x,
            } => format!(
                "\"event\":\"message_signed\",\"payload\":{}",
                export_complex_single(x, |a| a.success())
            ),
            Event::MessageSignError {
                sign_message_display: x,
            } => format!(
                "\"event\":\"message_sign_error\",\"payload\":{}",
                export_complex_single(x, |a| a.pwd_failure())
            ),
            Event::IdentityAdded {
                identity_history: x,
            } => format!(
                "\"event\":\"identity_added\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::IdentityRemoved {
                identity_history: x,
            } => format!(
                "\"event\":\"identity_removed\",\"payload\":{}",
                export_complex_single(x, |a| a.show())
            ),
            Event::IdentitiesWiped => String::from("\"event\":\"identities_wiped\""),
            Event::DeviceWasOnline => String::from("\"event\":\"device_online\""),
            Event::ResetDangerRecord => String::from("\"event\":\"reset_danger_record\""),
            Event::SeedCreated { seed_created: x } => {
                format!("\"event\":\"seed_created\",\"payload\":\"{}\"", x)
            }
            Event::SeedNameWasShown {
                seed_name_was_shown: seed_name,
            } => format!(
                "\"event\":\"seed_name_shown\",\"payload\":\"{}\"",
                seed_name
            ),
            Event::Warning { warning: x } => format!("\"event\":\"warning\",\"payload\":\"{}\"", x),
            Event::WrongPassword => String::from("\"event\":\"wrong_password_entered\""),
            Event::UserEntry { user_entry: x } => {
                format!("\"event\":\"user_entered_event\",\"payload\":\"{}\"", x)
            }
            Event::SystemEntry { system_entry: x } => {
                format!("\"event\":\"system_entered_event\",\"payload\":\"{}\"", x)
            }
            Event::HistoryCleared => String::from("\"event\":\"history_cleared\""),
            Event::DatabaseInitiated => String::from("\"event\":\"database_initiated\""),
        }
    }
}

#[cfg(feature = "signer")]
impl Entry {
    /// Print json with [`Entry`] for user interface
    ///
    /// Required input is the function to print `SignDisplay` contents.
    ///
    /// Currently general log view shows raw hexadecimal transaction,
    /// detailed log view shows parsed transaction if parsing is possible.
    pub fn show<O>(&self, op: O) -> String
    where
        O: Fn(&SignDisplay) -> String + Copy,
    {
        let events_chain = export_complex_vector(&self.events, |a| a.show(|b| op(b)));
        format!(
            "\"timestamp\":\"{}\",\"events\":{}",
            self.timestamp, events_chain
        )
    }
}

/// Test function generating a set of all possible events
///
/// Uses mock values and is needed to test json format in displaying all events
/// in user interface.  
#[cfg(feature = "signer")]
pub fn all_events_preview() -> Vec<Event> {
    let meta_values = MetaValues {
        name: String::from("westend"),
        version: 9000,
        optional_base58prefix: Some(42),
        warn_incomplete_extensions: false,
        meta: Vec::new(),
    };
    let public = [
        142, 175, 4, 21, 22, 135, 115, 99, 38, 201, 254, 161, 126, 37, 252, 82, 135, 97, 54, 147,
        201, 18, 144, 156, 178, 38, 170, 71, 148, 242, 106, 72,
    ];
    let verifier_value = VerifierValue::Standard {
        multi_signer: MultiSigner::Sr25519 {
            public: sp_core::sr25519::Public::from_raw(public).into(),
        },
    };
    let verifier = Verifier {
        verifier_value: Some(verifier_value.clone()),
    };
    let valid_current_verifier = ValidCurrentVerifier::General;
    let network_specs = NetworkSpecs {
        base58prefix: 42,
        color: String::from("#660D35"),
        decimals: 12,
        encryption: Encryption::Sr25519,
        genesis_hash: hex::decode(
            "e143f23803ac50e8f6f8e62695d1ce9e4e1d68aa36c1cd2cfd15340213f3423e",
        )
        .expect("known value")
        .try_into()
        .expect("known value"),
        logo: String::from("westend"),
        name: String::from("westend"),
        order: 3,
        path_id: String::from("//westend"),
        secondary_color: String::from("#262626"),
        title: String::from("Westend"),
        unit: String::from("WND"),
    };
    vec![
        Event::MetadataAdded {
            meta_values_display: MetaValuesDisplay::get(&meta_values),
        },
        Event::MetadataRemoved {
            meta_values_display: MetaValuesDisplay::get(&meta_values),
        },
        Event::MetadataSigned {
            meta_values_export: MetaValuesExport::get(&meta_values, &verifier_value),
        },
        Event::NetworkSpecsAdded {
            network_specs_display: NetworkSpecsDisplay::get(
                &network_specs,
                &valid_current_verifier,
                &verifier,
            ),
        },
        Event::NetworkSpecsRemoved {
            network_specs_display: NetworkSpecsDisplay::get(
                &network_specs,
                &valid_current_verifier,
                &verifier,
            ),
        },
        Event::NetworkSpecsSigned {
            network_specs_export: NetworkSpecsExport::get(
                &network_specs.to_send(),
                &verifier_value,
            ),
        },
        Event::NetworkVerifierSet {
            network_verifier_display: NetworkVerifierDisplay::get(
                &VerifierKey::from_parts(&network_specs.genesis_hash),
                &valid_current_verifier,
                &verifier,
            ),
        },
        Event::GeneralVerifierSet {
            verifier: verifier.to_owned(),
        },
        Event::TypesAdded {
            types_display: TypesDisplay::get(&ContentLoadTypes::from_slice(&[]), &verifier),
        },
        Event::TypesRemoved {
            types_display: TypesDisplay::get(&ContentLoadTypes::from_slice(&[]), &verifier),
        },
        Event::TypesSigned {
            types_export: TypesExport::get(&ContentLoadTypes::from_slice(&[]), &verifier_value),
        },
        Event::TransactionSigned {
            sign_display: SignDisplay::get(
                &Vec::new(),
                "westend",
                &verifier_value,
                "send to Alice",
            ),
        },
        Event::TransactionSignError {
            sign_display: SignDisplay::get(
                &Vec::new(),
                "westend",
                &verifier_value,
                "send to Alice",
            ),
        },
        Event::MessageSigned {
            sign_message_display: SignMessageDisplay::get(
                "This is Alice\nRoger",
                "westend",
                &verifier_value,
                "send to Alice",
            ),
        },
        Event::MessageSignError {
            sign_message_display: SignMessageDisplay::get(
                "This is Alice\nRoger",
                "westend",
                &verifier_value,
                "send to Alice",
            ),
        },
        Event::IdentityAdded {
            identity_history: IdentityHistory::get(
                "Alice",
                &Encryption::Sr25519,
                &public,
                "//",
                &network_specs.genesis_hash,
            ),
        },
        Event::IdentityRemoved {
            identity_history: IdentityHistory::get(
                "Alice",
                &Encryption::Sr25519,
                &public,
                "//",
                &network_specs.genesis_hash,
            ),
        },
        Event::IdentitiesWiped,
        Event::DeviceWasOnline,
        Event::ResetDangerRecord,
        Event::SeedCreated {
            seed_created: String::from("Alice"),
        },
        Event::SeedNameWasShown {
            seed_name_was_shown: String::from("AliceSecretSeed"),
        },
        Event::Warning {
            warning: String::from("Received network information is not verified."),
        },
        Event::WrongPassword,
        Event::UserEntry {
            user_entry: String::from("Lalala!!!"),
        },
        Event::SystemEntry {
            system_entry: String::from("Blip blop"),
        },
        Event::HistoryCleared,
        Event::DatabaseInitiated,
    ]
}

/// Test function generating a printed version of an entry with all possible
/// events
///
/// Uses mock values and is needed to test json format in displaying all events
/// in user interface.  
#[cfg(feature = "signer")]
pub fn print_all_events() -> String {
    let events = all_events_preview();
    let entry = Entry {
        timestamp: String::from("2019-12-15 12:00:0.00000000 UTC"),
        events,
    };
    format!(
        "{{\"order\":0,{}}}",
        entry.show(|a| format!("\"{}\"", hex::encode(a.transaction())))
    )
}
