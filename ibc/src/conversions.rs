use crate::header::Header;
use crate::misbehaviour::Misbehaviour;
use crate::state::{ClientState, ConsensusState};
use crate::wasm;
use crate::{convert_hash2root, convert_root2hash};
use crate::{Error, Kind};

use cosmwasm_std::Binary;
use std::convert::TryFrom;

///
impl<T> TryFrom<wasm::ConsensusState> for ConsensusState<T> {
    type Error = Error;
    fn try_from(ibc: wasm::ConsensusState) -> Result<Self, Self::Error> {
        let root = ibc.root.ok_or_else(|| Kind::MissingField {
            struct_name: String::from("wasm::ConsensusState"),
            field_name: String::from("root"),
        })?;
        let s = Self::from_raw(
            Binary::from(ibc.data),
            ibc.timestamp,
            convert_root2hash(root),
        );
        Ok(s)
    }
}

impl<T> From<ConsensusState<T>> for wasm::ConsensusState {
    fn from(cs: ConsensusState<T>) -> Self {
        let root = convert_hash2root(cs.root());
        Self {
            data: cs.data.0,
            timestamp: cs.timestamp,
            root: Some(root),
        }
    }
}

///
impl<T> TryFrom<wasm::ClientState> for ClientState<T> {
    type Error = Error;
    fn try_from(ibc: wasm::ClientState) -> Result<Self, Self::Error> {
        let latest_height = ibc.latest_height.ok_or_else(|| Kind::MissingField {
            struct_name: String::from("wasm::ClientState"),
            field_name: String::from("latest_height"),
        })?;
        let s = Self::from_raw(
            Binary::from(ibc.data),
            Binary::from(ibc.code_id),
            latest_height,
            None,
        );
        Ok(s)
    }
}

impl<T> From<ClientState<T>> for wasm::ClientState {
    fn from(cs: ClientState<T>) -> Self {
        Self {
            data: cs.data.0,
            code_id: cs.code_id.0,
            latest_height: Some(cs.latest_height),
        }
    }
}

///
impl<T> TryFrom<wasm::Header> for Header<T> {
    type Error = Error;
    fn try_from(ibc: wasm::Header) -> Result<Self, Self::Error> {
        let height = ibc.height.ok_or_else(|| Kind::MissingField {
            struct_name: String::from("wasm::Header"),
            field_name: String::from("height"),
        })?;

        let s = Self::from_raw(Binary::from(ibc.data), height);
        Ok(s)
    }
}
impl<T> From<Header<T>> for wasm::Header {
    fn from(h: Header<T>) -> Self {
        Self {
            data: h.data.0,
            height: Some(h.height),
        }
    }
}

///
impl<T> TryFrom<wasm::Misbehaviour> for Misbehaviour<T> {
    type Error = Error;
    fn try_from(ibc: wasm::Misbehaviour) -> Result<Self, Self::Error> {
        let head1 = ibc.header_1.ok_or_else(|| Kind::MissingField {
            struct_name: String::from("wasm::Misbehaviour"),
            field_name: String::from("header_1"),
        })?;
        let head2 = ibc.header_2.ok_or_else(|| Kind::MissingField {
            struct_name: String::from("wasm::Misbehaviour"),
            field_name: String::from("header_1"),
        })?;

        let s = Self {
            client_id: ibc.client_id,
            header_1: Header::try_from(head1)?,
            header_2: Header::try_from(head2)?,
        };
        Ok(s)
    }
}

impl<T> From<Misbehaviour<T>> for wasm::Misbehaviour {
    fn from(mis: Misbehaviour<T>) -> Self {
        Self {
            client_id: mis.client_id,
            header_1: Some(mis.header_1.into()),
            header_2: Some(mis.header_2.into()),
        }
    }
}
