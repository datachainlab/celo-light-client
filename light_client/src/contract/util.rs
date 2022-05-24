use crate::contract::errors::{convert_celo, convert_rlp};
use crate::contract::{CeloClientState, WasmClientState, WasmConsensusState, WasmHeader};

use celo_ibc::header::extract_header;
use celo_ibc::state::{extract_client, extract_consensus};
use celo_types::state::State as CeloState;

use ibc_proto::ibc::core::client::v1::Height;

use cosmwasm_std::{attr, to_binary, StdError, StdResult, HandleResponse};
use serde::Serialize;
use std::fmt::Display;

pub(crate) fn to_generic_err<T>(err: T) -> StdError
where
    T: Display,
{
    StdError::GenericErr {
        msg: err.to_string(),
    }
}

pub(crate) fn wrap_response<T>(result: T, action: &'static str) -> StdResult<HandleResponse>
where
    T: Serialize,
{
    let response_data = to_binary(&result)?;
    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![attr("action", action)],
        data: Some(response_data),
    })
}
pub(crate) fn wrap_response_with_height<T>(
    result: T,
    action: &'static str,
    height: &Height,
) -> StdResult<HandleResponse>
where
    T: Serialize,
{
    let response_data = to_binary(&result)?;

    Ok(HandleResponse {
        messages: vec![],
        attributes: vec![
            attr("action", action),
            attr("latest_height", format!("{:?}", height))],
        data: Some(response_data),
    })
}

pub(crate) fn check_misbehaviour_header(
    _num: u16,
    me: &WasmClientState,
    consensus_state: &WasmConsensusState,
    header: &WasmHeader,
) -> Result<(), StdError> {
    // Unmarshal entries
    let celo_consensus =
        extract_consensus(consensus_state).map_err(|e| convert_rlp(e, "CeloConsensusState"))?;
    let celo_client: CeloClientState =
        extract_client(me).map_err(|e| convert_rlp(e, "CeloClientState"))?;
    let celo_header = extract_header(header).map_err(|e| convert_rlp(e, "CeloHeader"))?;
    // Verify header
    let state: CeloState<CeloClientState> = CeloState::new(celo_consensus, &celo_client);
    state.verify_header_seal(&celo_header).map_err(convert_celo)
}

pub(crate) fn is_expired(
    current_timestamp: u64,
    latest_timestamp: u64,
    trusting_period: u64,
) -> bool {
    current_timestamp > latest_timestamp + trusting_period
}
