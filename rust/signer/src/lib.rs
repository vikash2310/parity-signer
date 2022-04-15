// Copyright 2015-2021 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

mod ffi_types;

use crate::ffi_types::*;
use std::fmt::Display;

#[derive(Debug)]
pub enum ErrorDisplayed {
    Str { s: String },
}

impl From<anyhow::Error> for ErrorDisplayed {
    fn from(e: anyhow::Error) -> Self {
        Self::Str {
            s: format!("error on signer side: {}", e),
        }
    }
}

impl Display for ErrorDisplayed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TODO")
    }
}

include!(concat!(env!("OUT_DIR"), "/signer.uniffi.rs"));

fn action_get_name(action: &Action) -> String {
    match action {
        Action::NavbarLog => "Log",
        Action::NavbarScan => "Scan",
        Action::NavbarKeys => "Keys",
        Action::NavbarSettings => "Settings",
        Action::GoBack => "<",
        _ => "",
    }
    .to_string()
}

fn backend_action(action: Action, details: &str, seed_phrase: &str) -> String {
    navigator::do_action(action, details, seed_phrase)
}

fn init_navigation(dbname: &str, seed_names: &str) {
    navigator::init_navigation(dbname, seed_names)
}

fn update_seed_names(seed_names: &str) {
    navigator::update_seed_names(seed_names)
}

fn qrparser_get_packets_total(data: &str, cleaned: bool) -> anyhow::Result<u32, ErrorDisplayed> {
    qr_reader_phone::get_length(data, cleaned).map_err(Into::into)
}

fn qrparser_try_decode_qr_sequence(
    data: &str,
    cleaned: bool,
) -> anyhow::Result<String, anyhow::Error> {
    qr_reader_phone::decode_sequence(data, cleaned)
}

fn substrate_path_check(seed_name: &str, path: &str, network: &str, dbname: &str) -> String {
    db_handling::interface_signer::dynamic_path_check(dbname, seed_name, path, network)
}

fn history_init_history_with_cert(dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::cold_default::signer_init_with_cert(dbname).map_err(|e| e.anyhow())
}

fn history_init_history_no_cert(dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::cold_default::signer_init_no_cert(dbname).map_err(|e| e.anyhow())
}

fn history_device_was_online(dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::manage_history::device_was_online(dbname).map_err(|e| e.anyhow())
}

fn history_get_warnings(dbname: &str) -> anyhow::Result<bool, anyhow::Error> {
    db_handling::helpers::get_danger_status(dbname).map_err(|e| e.anyhow())
}

fn history_acknowledge_warnings(dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::manage_history::reset_danger_status_to_safe(dbname).map_err(|e| e.anyhow())
}

fn history_entry_system(event: Event, dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::manage_history::history_entry_system(dbname, event.into()).map_err(|e| e.anyhow())
}

fn history_seed_name_was_shown(seed_name: &str, dbname: &str) -> anyhow::Result<(), anyhow::Error> {
    db_handling::manage_history::seed_name_was_shown(dbname, seed_name.to_string())
        .map_err(|e| e.anyhow())
}

fn get_all_tx_cards() -> String {
    if let transaction_parsing::Action::Read(content) =
        transaction_parsing::test_all_cards::make_all_cards()
    {
        format!("{{{}}}", content)
    } else {
        "".to_string()
    }
}

fn get_all_log_cards() -> String {
    definitions::history::print_all_events()
}

ffi_support::define_string_destructor!(signer_destroy_string);

#[cfg(test)]
mod tests {
    //use super::*;
}
