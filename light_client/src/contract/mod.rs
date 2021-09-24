#![allow(dead_code)]

pub mod msg;
mod types;
pub use msg::*;
mod errors;
mod serialization;
mod state;
mod store;
mod util;
mod wasm;

use crate::contract::serialization::*;
use crate::contract::store::*;
use crate::contract::wasm::*;
use crate::contract::util::*;
use celo_types::state::State;

use celo_ibc::{extract_header, extract_lc_client_state, extract_lc_consensus_state};
use celo_ibc::{
    Channel, ClientState, ConnectionEnd, ConsensusState, Header, Height, MerklePrefix, MerkleRoot,
    Misbehaviour,
};
use celo_types::header::Header as CeloHeader;
use celo_types::{client::LightClientState, consensus::LightConsensusState};

use cosmwasm_std::{attr, to_binary, to_vec, Binary};
use cosmwasm_std::{Deps, DepsMut, Env, MessageInfo};
use cosmwasm_std::{QueryResponse, Response, StdError, StdResult};
//use prost::Message;

// # A few notes on certain design decisions
// ## Serialization
// RLP is being used in a few methods, why can't we use JSON everywhere?
//
// CosmWasm doesn't accept floating point operations (see: `cosmwasm/packages/vm/src/middleware/deterministic.rs`)
// and that's for a good reason. Even if you're not using floating point arithmetic explicilty,
// some other library might do it behind the scenes. That's exactly what happens with serde json.
//
// For example to deserialize Celo `Header` type, a set of fields needs to be translated from
// String to Int/BigInt (serialized message comes from celo-geth daemon). The following line would
// implicitly use floating point arithmetic:
// ```
// Source: src/serialization/bytes.rs
// let s: &str = Deserialize::deserialize(deserializer)?;
// ```
//
// How can I check if my wasm binary uses floating points?
// * gaia will fail to upload wasm code (validation will fail)
// * run: `wasm2wat target/wasm32-unknown-unknown/release/celo_light_client.wasm  | grep f64`
//
// Taken all the possible options I think the easiest way is to use RLP for the structs that fail
// to serialize/deserialize via JSON (ie. Header, LightConsensusState)
//
// ## IBC
// ### Proof
// ICS-23 specifies the generic proof structure (ie. ExistenceProof). Without the other side of the
// bridge (CosmosLC on CeloBlockchain) we can't say for sure what the proof structure is going to
// be (TendermintProof, MerkleProof etc.) for sure.
//
// I've used MerkleProof + MerklePrefix as a placeholder to be revisited once we have the other side of the bridge
// implemented
//
// ### Counterparty Consensus State
// Essentially this is Cosmos/Tendermint consensus state coming from the other side of the bridge. For now it's almost empty datastructure,
// use as a placeholder.
//
// ### Serialization
// I assumed that proof and counterparty_consensus_state are encoded with JsonMarshaller.
// It's likely that amino / protobuf binary encoding will be used...
//
// ### Vocabulary (hint for the reader)
// CeloLC on CosmosNetwork:
// * proof - proof that CosmosConsensusState is stored on the TendermintLC in CeloBlockchain
// * counterparty_consensus_state - CosmosConsensusState
//
// Tendermint LC on Celo Blockchain:
// * proof - proof that CeloConsensusState is stored on CeloLC in CosmosNetwork
// * counterparty_consensus_state - CeloConsensusState

pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: HandleMsg,
) -> StdResult<Response> {
    // The 10-wasm Init method is split into two calls, where the second (via handle())
    // call expects ClientState included in the return.
    //
    // Therefore it's better to execute whole logic in the second call.
    Ok(Response::default())
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: HandleMsg,
) -> Result<Response, StdError> {
    match msg {
        HandleMsg::InitializeState {
            consensus_state,
            me,
        } => init_contract(deps, env, info, consensus_state, me),
        HandleMsg::CheckHeaderAndUpdateState {
            header,
            consensus_state,
            me,
        } => check_header_and_update_state(deps, env, me, consensus_state, header),
        HandleMsg::CheckMisbehaviourAndUpdateState {
            me,
            misbehaviour,
            consensus_state_1,
            consensus_state_2,
        } => check_misbehaviour(
            deps,
            env,
            me,
            misbehaviour,
            consensus_state_1,
            consensus_state_2,
        ),
        HandleMsg::VerifyUpgradeAndUpdateState {
            me,
            new_client_state,
            new_consensus_state,
            client_upgrade_proof,
            consensus_state_upgrade_proof,
            last_height_consensus_state,
        } => verify_upgrade_and_update_state(
            deps,
            env,
            me,
            new_client_state,
            new_consensus_state,
            client_upgrade_proof,
            consensus_state_upgrade_proof,
            last_height_consensus_state,
        ),
        HandleMsg::CheckSubstituteAndUpdateState {
            me,
            substitute_client_state,
            subject_consensus_state,
            initial_height,
        } => check_substitute_client_state(
            deps,
            env,
            me,
            substitute_client_state,
            subject_consensus_state,
            initial_height,
        ),
        HandleMsg::ZeroCustomFields { me } => zero_custom_fields(deps, env, me),
    }
}
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::VerifyClientState {
            me,
            height,
            commitment_prefix,
            counterparty_client_identifier,
            proof,
            counterparty_client_state,
            consensus_state,
        } => verify_client_state(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            counterparty_client_identifier,
            proof,
            counterparty_client_state,
            consensus_state,
        ),
        QueryMsg::VerifyClientConsensusState {
            me,
            height,
            consensus_height,
            commitment_prefix,
            counterparty_client_identifier,
            proof,
            counterparty_consensus_state,
            consensus_state,
        } => verify_client_consensus_state(
            deps,
            env,
            me,
            height,
            consensus_height,
            commitment_prefix,
            counterparty_client_identifier,
            proof,
            counterparty_consensus_state,
            consensus_state,
        ),
        QueryMsg::VerifyConnectionState {
            me,
            height,
            commitment_prefix,
            proof,
            connection_id,
            connection_end,
            consensus_state,
        } => verify_connection_state(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            connection_id,
            connection_end,
            consensus_state,
        ),
        QueryMsg::VerifyChannelState {
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            channel,
            consensus_state,
        } => verify_channel_state(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            channel,
            consensus_state,
        ),
        QueryMsg::VerifyPacketCommitment {
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            commitment_bytes,
            consensus_state,
        } => verify_packet_commitment(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            commitment_bytes,
            consensus_state,
        ),
        QueryMsg::VerifyPacketAcknowledgement {
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            acknowledgement,
            consensus_state,
        } => verify_packet_acknowledgment(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            acknowledgement,
            consensus_state,
        ),
        QueryMsg::VerifyPacketReceiptAbsence {
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            consensus_state,
        } => verify_packet_receipt_absence(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            sequence,
            consensus_state,
        ),
        QueryMsg::VerifyNextSequenceRecv {
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            next_sequence_recv,
            consensus_state,
        } => verify_next_sequence_recv(
            deps,
            env,
            me,
            height,
            commitment_prefix,
            proof,
            port_id,
            channel_id,
            delay_time_period,
            delay_block_period,
            next_sequence_recv,
            consensus_state,
        ),
        QueryMsg::ProcessedTime { height } => {
            let processed_time = get_processed_time(deps.storage, EMPTY_PREFIX, &height)?;
            to_binary(&ProcessedTimeResponse {
                time: processed_time,
            })
        }
        QueryMsg::Status {
            me,
            consensus_state,
        } => status(deps, env, me, consensus_state),
    }
}

fn init_contract(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    consensus_state: ConsensusState,
    me: ClientState,
) -> StdResult<Response> {
    // Unmarshal initial state entry (ie. validator set, epoch_size etc.)
    let light_consensus_state =
        extract_lc_consensus_state(&consensus_state).map_err(|e| StdError::ParseErr {
            target_type: String::from("LightConsensusState"),
            msg: format!("{}", e),
        })?;
    // Verify initial state
    if let Err(e) = light_consensus_state.verify() {
        return Err(StdError::generic_err(format!(
            "Initial state verification failed. Error: {}",
            e
        )));
    }
    // Set metadata for initial consensus state
    set_consensus_meta(&env, deps.storage, EMPTY_PREFIX, &me.latest_height)?;
    // Update the state
    let response_data = to_binary(&InitializeStateResult {
        me,
        result: ClientStateCallResponseResult::success(),
    })?;
    let response = Response::new()
        .add_attributes(vec![
            ("action", "init_block"),
            (
                "last_consensus_state_height",
                &light_consensus_state.number.to_string(),
            ),
        ])
        .set_data(response_data);
    Ok(response)
}

fn check_header_and_update_state(
    deps: DepsMut,
    env: Env,
    me: ClientState,
    consensus_state: ConsensusState,
    wasm_header: Header,
) -> StdResult<Response> {
    let current_timestamp: u64 = env.block.time.seconds();
    // Unmarshal header
    let header: CeloHeader = extract_header(&wasm_header).map_err(|e| StdError::ParseErr {
        target_type: String::from("CeloHeader"),
        msg: format!("{}", e),
    })?;
    // Unmarshal state entry
    let light_consensus_state =
        extract_lc_consensus_state(&consensus_state).map_err(|e| StdError::ParseErr {
            target_type: String::from("LightConsensusState"),
            msg: format!("{}", e),
        })?;
    // Unmarshal state config
    let light_client_state = extract_lc_client_state(&me).map_err(|e| StdError::ParseErr {
        target_type: String::from("LightClientState"),
        msg: format!("{}", e),
    })?;
    // Ingest new header
    let mut state: State = State::new(light_consensus_state, &light_client_state);
    if let Err(e) = state.insert_header(&header, current_timestamp) {
        return Err(StdError::generic_err(format!(
            "Unable to ingest header. Error: {}",
            e
        )));
    }
    // Update the state
    let new_client_state = me;
    let new_consensus_state = ConsensusState::new(
        state.snapshot(),
        consensus_state.code_id,
        header.time.as_u64(),
        MerkleRoot::from(header.root),
    );
    // set metadata for this consensus state
    set_consensus_meta(&env, deps.storage, EMPTY_PREFIX, &wasm_header.height)?;

    let response_data = to_binary(&CheckHeaderAndUpdateStateResult {
        new_client_state,
        new_consensus_state,
        result: ClientStateCallResponseResult::success(),
    })?;

    let response = Response::new()
        .add_attributes(vec![
            ("action", "update_block"),
            (
                "last_consensus_state_height",
                &state.snapshot().number.to_string(),
            ),
        ])
        .set_data(response_data);
    Ok(response)
}

pub(crate) fn verify_upgrade_and_update_state(
    deps: DepsMut,
    env: Env,
    me: ClientState,
    new_client_state: ClientState,
    new_consensus_state: ConsensusState,
    client_upgrade_proof: String,
    consensus_state_upgrade_proof: String,
    last_height_consensus_state: ConsensusState,
) -> StdResult<Response> {
    //let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Sanity check
    if new_client_state.latest_height <= me.latest_height {
        return Err(StdError::generic_err(format!(
            "upgraded client height {:?} must be at greater than current client height {:?}",
            new_client_state.latest_height, me.latest_height
        )));
    }

    // Unmarshal state config
    let light_client_state = extract_lc_client_state(&me).map_err(|e| StdError::ParseErr {
        target_type: String::from("LightClientState"),
        msg: format!("{}", e),
    })?;

    /*
    // Unmarshal proofs
    let proof_client: MerkleProof =
    from_base64_json_slice(&client_upgrade_proof, "msg.client_proof")?;
    let proof_consensus: MerkleProof =
    from_base64_json_slice(&consensus_state_upgrade_proof, "msg.consensus_proof")?;
    */

    // Unmarshal root
    let root: Vec<u8> = from_base64(
        &last_height_consensus_state.root.hash,
        "msg.last_height_consensus_state.root",
    )?;

    // Check consensus state expiration
    let current_timestamp: u64 = env.block.time.seconds();
    if is_expired(
        current_timestamp,
        last_height_consensus_state.timestamp,
        &light_client_state,
    ) {
        return Err(StdError::generic_err("cannot upgrade an expired client"));
    }

    // Verify client proof
    // TODO!!!
    /*
    let value: Vec<u8> = to_vec(&new_client_state)?;
    let upgrade_client_path = construct_upgrade_merkle_path(
    &light_client_state.upgrade_path,
    ClientUpgradePath::UpgradedClientState(me.latest_height.unwrap().revision_number),
    );
    if !verify_membership(
    &proof_consensus,
    &specs,
    &root,
    &upgrade_client_path,
    value,
    0,
    )? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Verify consensus proof
    // TODO!!!
    /*
    let value: Vec<u8> = to_vec(&new_consensus_state)?;
    let upgrade_consensus_state_path = construct_upgrade_merkle_path(
    &light_client_state.upgrade_path,
    ClientUpgradePath::UpgradedClientConsensusState(me.latest_height.unwrap().revision_number),
    );
    if !verify_membership(
    &proof_client,
    &specs,
    &root,
    &upgrade_consensus_state_path,
    value,
    0,
    )? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // set metadata for this consensus state
    set_consensus_meta(
        &env,
        deps.storage,
        EMPTY_PREFIX,
        &new_client_state.latest_height,
    )?;

    // Build up the response
    wrap_response(
        &VerifyUpgradeAndUpdateStateResult {
            result: ClientStateCallResponseResult::success(),
            // NOTE: The contents of client or consensus state
            // are subject to change (once we have end-to-end test flow)
            new_client_state,
            new_consensus_state,
        },
        "verify_upgrade_and_update_state",
    )
}

pub(crate) fn check_misbehaviour(
    _deps: DepsMut,
    _env: Env,
    me: ClientState,
    misbehaviour: Misbehaviour,
    consensus_state1: ConsensusState,
    consensus_state2: ConsensusState,
) -> StdResult<Response> {
    // The header heights are expected to be the same
    if misbehaviour.header_1.height != misbehaviour.header_2.height {
        return Err(StdError::generic_err(format!(
            "Misbehaviour header heights differ, {:?} != {:?}",
            misbehaviour.header_1.height, misbehaviour.header_2.height
        )));
    }

    // If client is already frozen at earlier height than misbehaviour, return with error
    if me.frozen_height.is_some()
        && me.frozen_height.as_ref().unwrap() <= &misbehaviour.header_1.height
    {
        return Err(StdError::generic_err(format!(
            "Client is already frozen at earlier height {:?} than misbehaviour height {:?}",
            me.frozen_height.unwrap(),
            misbehaviour.header_1.height
        )));
    }

    // Unmarshal header
    let header_1 = extract_header(&misbehaviour.header_1).map_err(|e| StdError::ParseErr {
        target_type: String::from("CeloHeader"),
        msg: format!("{}", e),
    })?;
    let header_2 = extract_header(&misbehaviour.header_2).map_err(|e| StdError::ParseErr {
        target_type: String::from("CeloHeader"),
        msg: format!("{}", e),
    })?;

    // The header state root should differ
    if header_1.root == header_2.root {
        return Err(StdError::generic_err(
            "Header's state roots should differ, but are the same",
        ));
    }

    // Check the validity of the two conflicting headers against their respective
    // trusted consensus states
    check_misbehaviour_header(1, &me, &consensus_state1, &misbehaviour.header_1)?;
    check_misbehaviour_header(2, &me, &consensus_state2, &misbehaviour.header_2)?;

    // Store the new state
    let mut new_client_state = me;
    new_client_state.frozen_height = Some(misbehaviour.header_1.height.clone());

    let response_data = Binary(to_vec(&CheckMisbehaviourAndUpdateStateResult {
        new_client_state,
        result: ClientStateCallResponseResult::success(),
    })?);

    let resp = Response::new()
        .add_attributes(vec![
            attr("action", "verify_membership"),
            attr("height", format!("{:?}", misbehaviour.header_1.height)),
        ])
        .set_data(response_data);
    Ok(resp)
}

// zero_custom_fields returns a ClientState that is a copy of the current ClientState
// with all client customizable fields zeroed out
pub(crate) fn zero_custom_fields(
    _deps: DepsMut,
    _env: Env,
    me: ClientState,
) -> StdResult<Response> {
    let mut new_client_state = ClientState::default();
    new_client_state.data = me.data;

    // Build up the response
    wrap_response(
        &ZeroCustomFieldsResult {
            me: new_client_state,
        },
        "zero_custom_fields",
    )
}

pub(crate) fn check_misbehaviour_header(
    num: u16,
    me: &ClientState,
    consensus_state: &ConsensusState,
    header: &Header,
) -> Result<(), StdError> {
    // Unmarshal state entry
    let light_consensus_state =
        extract_lc_consensus_state(&consensus_state).map_err(|e| StdError::ParseErr {
            target_type: String::from("LightConsensusState"),
            msg: format!("{}", e),
        })?;

    // Unmarshal state config
    let light_client_state = extract_lc_client_state(&me).map_err(|e| StdError::ParseErr {
        target_type: String::from("LightClientState"),
        msg: format!("{}", e),
    })?;
    // Unmarshal header
    let celo_header: CeloHeader = extract_header(&header).map_err(|e| StdError::ParseErr {
        target_type: String::from("CeloHeader"),
        msg: format!("{}", e),
    })?;

    // Verify header
    let state: State = State::new(light_consensus_state, &light_client_state);
    match state.verify_header_seal(&celo_header) {
        Err(e) => {
            return Err(StdError::generic_err(format!(
                "Failed to verify header num: {} against it's consensus state. Error: {}",
                num, e
            )))
        }
        _ => return Ok(()),
    }
}

pub(crate) fn verify_client_state(
    _deps: Deps,
    _env: Env,
    _me: ClientState,
    _height: Height,
    commitment_prefix: MerklePrefix,
    counterparty_client_identifier: String,
    proof: String,
    counterparty_client_state: CosmosClientState,
    proving_consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!!!
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    //let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(
    &proving_consensus_state.root.hash,
    "msg.proving_consensus_state.root",
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let client_prefixed_path = IcsPath::ClientState(
    ClientId::from_str(&counterparty_client_identifier).map_err(to_generic_err)?,
    )
    .to_string();

    // Verify proof against key-value pair
    let path = apply_prefix(&commitment_prefix, vec![client_prefixed_path])?;
    let value: Vec<u8> = to_vec(&counterparty_client_state)?;

    if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyClientStateResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_client_consensus_state(
    _deps: Deps,
    _env: Env,
    _me: ClientState,
    _height: Height,
    consensus_height: Height,
    commitment_prefix: MerklePrefix,
    counterparty_client_identifier: String,
    proof: String,
    counterparty_consensus_state: CosmosConsensusState,
    proving_consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!!
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(
    &proving_consensus_state.root.hash,
    "msg.proving_consensus_state.root",
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let client_prefixed_path = IcsPath::ClientConsensusState {
    client_id: ClientId::from_str(&counterparty_client_identifier).map_err(to_generic_err)?,
    epoch: consensus_height.revision_number,
    height: consensus_height.revision_height,
    }
    .to_string();

    // Verify proof against key-value pair
    let path = apply_prefix(&commitment_prefix, vec![client_prefixed_path])?;
    let value: Vec<u8> = to_vec(&counterparty_consensus_state)?;

    if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyClientConsensusStateResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_connection_state(
    _deps: Deps,
    _env: Env,
    _me: ClientState,
    _height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    connection_id: String,
    connection_end: ConnectionEnd,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(
    &consensus_state.root.hash,
    "msg.proving_consensus_state.root",
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let connection_path =
    IcsPath::Connections(ConnectionId::from_str(&connection_id).map_err(to_generic_err)?)
    .to_string();

    // Verify proof against key-value pair
    let path = apply_prefix(&commitment_prefix, vec![connection_path])?;
    let value: Vec<u8> = to_vec(&connection_end)?;

    if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyConnectionStateResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_channel_state(
    _deps: Deps,
    _env: Env,
    _me: ClientState,
    _height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    port_id: String,
    channel_id: String,
    channel: Channel,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(
    &consensus_state.root.hash,
    "msg.proving_consensus_state.root",
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let channel_path = IcsPath::ChannelEnds(
    PortId::from_str(&port_id).map_err(to_generic_err)?,
    ChannelId::from_str(&channel_id).map_err(to_generic_err)?,
    )
    .to_string();

    // Verify proof against key-value pair
    let path = apply_prefix(&commitment_prefix, vec![channel_path])?;
    let value: Vec<u8> = to_vec(&channel)?;

    if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyChannelStateResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_packet_commitment(
    deps: Deps,
    env: Env,
    _me: ClientState,
    height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    port_id: String,
    channel_id: String,
    delay_time_period: u64,
    delay_block_period: u64,
    sequence: u64,
    commitment_bytes: String,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!
    /*
        // Unmarshal proof
        let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
        let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

        // Get root from proving (celo) consensus state
        let root: Vec<u8> = from_base64(&consensus_state.root.hash, "msg.consensus_state.root")?;

        // Check delay period has passed
        verify_delay_period_passed(
        deps,
        height,
        env.block.height,
        env.block.time,
        delay_time_period,
        delay_block_period,
        )?;

        // Build path (proof is used to validate the existance of value under that path)
        let commitment_path = IcsPath::Commitments {
        port_id: PortId::from_str(&port_id).map_err(to_generic_err)?,
        channel_id: ChannelId::from_str(&channel_id).map_err(to_generic_err)?,
        sequence: Sequence::from(sequence),
        }
        .to_string();

        // Verify proof against key-value pair
        let path = apply_prefix(&commitment_prefix, vec![commitment_path])?;
        let value: Vec<u8> = from_base64(&commitment_bytes, "msg.commitment_bytes")?;

        if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
        return Err(StdError::generic_err(
        "proof membership verification failed (invalid proof)",
        ));
        }

    */
    // Build up the response
    to_binary(&VerifyPacketCommitmentResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_packet_acknowledgment(
    deps: Deps,
    env: Env,
    _me: ClientState,
    height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    port_id: String,
    channel_id: String,
    delay_time_period: u64,
    delay_block_period: u64,
    sequence: u64,
    acknowledgement: String,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!!
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(&consensus_state.root.hash, "msg.consensus_state.root")?;

    // Check delay period has passed
    verify_delay_period_passed(
    deps,
    height,
    env.block.height,
    env.block.time,
    delay_time_period,
    delay_block_period,
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let ack_path = IcsPath::Acks {
    port_id: PortId::from_str(&port_id).map_err(to_generic_err)?,
    channel_id: ChannelId::from_str(&channel_id).map_err(to_generic_err)?,
    sequence: Sequence::from(sequence),
    }
    .to_string();

    // Verify proof against key-value pair
    let path = apply_prefix(&commitment_prefix, vec![ack_path])?;
    let value: Vec<u8> = from_base64(&acknowledgement, "msg.acknowledgement")?;

    if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
    return Err(StdError::generic_err(
    "proof membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyPacketAcknowledgementResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_packet_receipt_absence(
    deps: Deps,
    env: Env,
    _me: ClientState,
    height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    port_id: String,
    channel_id: String,
    delay_time_period: u64,
    delay_block_period: u64,
    sequence: u64,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO !
    /*
    // Unmarshal proof
    let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
    let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

    // Get root from proving (celo) consensus state
    let root: Vec<u8> = from_base64(&consensus_state.root.hash, "msg.consensus_state.root")?;

    // Check delay period has passed
    verify_delay_period_passed(
    deps,
    height,
    env.block.height,
    env.block.time,
    delay_time_period,
    delay_block_period,
    )?;

    // Build path (proof is used to validate the existance of value under that path)
    let reciept_path = IcsPath::Receipts {
    port_id: PortId::from_str(&port_id).map_err(to_generic_err)?,
    channel_id: ChannelId::from_str(&channel_id).map_err(to_generic_err)?,
    sequence: Sequence::from(sequence),
    }
    .to_string();

    // Apply prefix
    let path = apply_prefix(&commitment_prefix, vec![reciept_path])?;

    // Verify single proof against key-value pair
    let key: &[u8] = path.key_path.last().unwrap().as_bytes();

    // Reference: cosmos-sdk/x/ibc/core/23-commitment/types/merkle.go
    // TODO: ics23-rs library doesn't seem to offer subroot calculation for non_exist
    if !ics23::verify_non_membership(&proof.proofs[0], &specs[0], &root, key) {
    return Err(StdError::generic_err(
    "proof non membership verification failed (invalid proof)",
    ));
    }
    */

    // Build up the response
    to_binary(&VerifyPacketReceiptAbsenceResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn verify_next_sequence_recv(
    deps: Deps,
    env: Env,
    _me: ClientState,
    height: Height,
    commitment_prefix: MerklePrefix,
    proof: String,
    port_id: String,
    channel_id: String,
    delay_time_period: u64,
    delay_block_period: u64,
    next_sequence_recv: u64,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    // TODO!
    /*
        // Unmarshal proof
        let proof: MerkleProof = from_base64_json_slice(&proof, "msg.proof")?;
        let specs = vec![ics23::iavl_spec(), ics23::tendermint_spec()];

        // Get root from proving (celo) consensus state
        let root: Vec<u8> = from_base64(&consensus_state.root.hash, "msg.consensus_state.root")?;

        // Check delay period has passed
        verify_delay_period_passed(
        deps,
        height,
        env.block.height,
        env.block.time,
        delay_time_period,
        delay_block_period,
        )?;

        // Build path (proof is used to validate the existance of value under that path)
        let next_sequence_recv_path = IcsPath::SeqRecvs(
        PortId::from_str(&port_id).map_err(to_generic_err)?,
        ChannelId::from_str(&channel_id).map_err(to_generic_err)?,
        )
        .to_string();

        // Verify proof against key-value pair
        let path = apply_prefix(&commitment_prefix, vec![next_sequence_recv_path])?;
        let value: Vec<u8> = u64_to_big_endian(next_sequence_recv);

        if !verify_membership(&proof, &specs, &root, &path, value, 0)? {
        return Err(StdError::generic_err(
        "proof membership verification failed (invalid proof)",
        ));
        }

    */

    // Build up the response
    to_binary(&VerifyPacketAcknowledgementResult {
        result: ClientStateCallResponseResult::success(),
    })
}

pub(crate) fn check_substitute_client_state(
    deps: DepsMut,
    env: Env,
    me: ClientState,
    substitute_client_state: ClientState,
    subject_consensus_state: ConsensusState,
    initial_height: Height,
) -> StdResult<Response> {
    if substitute_client_state.latest_height != initial_height {
        return Err(StdError::generic_err(format!(
           "substitute client revision number must equal initial height revision number ({:?} != {:?})",
           me.latest_height, initial_height
           )));
    }

    // unmarshal subject state
    let light_subject_client_state =
        extract_lc_client_state(&me).map_err(|e| StdError::ParseErr {
            target_type: String::from("LightClientState"),
            msg: format!("{}", e),
        })?;

    if me.frozen_height.is_some() && !light_subject_client_state.allow_update_after_misbehavior {
        return Err(StdError::generic_err(
            "client is not allowed to be unfrozen",
        ));
    }

    // unmarshal substitute state
    let light_substitute_client_state =
        extract_lc_client_state(&substitute_client_state).map_err(|e| StdError::ParseErr {
            target_type: String::from("LightClientState"),
            msg: format!("{}", e),
        })?;

    if light_substitute_client_state != light_subject_client_state {
        return Err(StdError::generic_err(
            "subject client state does not match substitute client state",
        ));
    }

    let current_timestamp: u64 = env.block.time.seconds();
    if is_expired(
        current_timestamp,
        subject_consensus_state.timestamp,
        &light_subject_client_state,
    ) && !light_subject_client_state.allow_update_after_expiry
    {
        return Err(StdError::generic_err(
            "client is not allowed to be unexpired",
        ));
    }

    let mut new_client_state = me.clone();
    new_client_state.frozen_height = None;

    // Copy consensus states and processed time from substitute to subject
    // starting from initial height and ending on the latest height (inclusive)
    let latest_height = substitute_client_state.latest_height.clone();
    for i in initial_height.revision_height..latest_height.revision_height + 1 {
        let height = Height {
            revision_height: i,
            revision_number: latest_height.revision_number,
        };

        let cs_bytes = get_consensus_state(deps.storage, SUBSTITUTE_PREFIX, &height);
        if cs_bytes.is_ok() {
            set_consensus_state(deps.storage, SUBJECT_PREFIX, &height, &cs_bytes.unwrap())?;
        }

        set_consensus_meta(&env, deps.storage, SUBJECT_PREFIX, &height)?;
    }

    new_client_state.latest_height = substitute_client_state.latest_height;

    let latest_consensus_state_bytes =
        get_consensus_state(deps.storage, SUBJECT_PREFIX, &me.latest_height)?;

    let latest_consensus_state = PartialConsensusState::default();
    //let latest_consensus_state =
    //PartialConsensusState::decode(latest_consensus_state_bytes.as_slice()).unwrap();
    let latest_light_consensus_state: LightConsensusState =
        rlp::decode(&latest_consensus_state.data).map_err(to_generic_err)?;

    if is_expired(
        current_timestamp,
        latest_light_consensus_state.timestamp,
        &light_subject_client_state,
    ) {
        return Err(StdError::generic_err("updated subject client is expired"));
    }

    wrap_response(
        &CheckSubstituteAndUpdateStateResult {
            result: ClientStateCallResponseResult::success(),
            new_client_state,
        },
        "check_substitute_and_update_state",
    )
}

fn status(
    _deps: Deps,
    env: Env,
    me: ClientState,
    consensus_state: ConsensusState,
) -> StdResult<QueryResponse> {
    let current_timestamp: u64 = env.block.time.seconds();
    let mut status = Status::Active;

    // Unmarshal state config
    let light_client_state = extract_lc_client_state(&me).map_err(|e| StdError::ParseErr {
        target_type: String::from("LightClientState"),
        msg: format!("{}", e),
    })?;

    if me.frozen_height.is_some() {
        status = Status::Frozen;
    } else {
        // Unmarshal state entry
        let light_consensus_state =
            extract_lc_consensus_state(&consensus_state).map_err(|e| StdError::ParseErr {
                target_type: String::from("LightConsensusState"),
                msg: format!("{}", e),
            })?;

        if is_expired(
            current_timestamp,
            light_consensus_state.timestamp,
            &light_client_state,
        ) {
            status = Status::Exipred;
        }
    }

    // Build up the response
    to_binary(&StatusResult { status })
}

// verify_delay_period_passed will ensure that at least delayPeriod amount of time has passed since consensus state was submitted
// before allowing verification to continue
fn verify_delay_period_passed(
    deps: Deps,
    proof_height: Height,
    current_height: u64,
    current_timestamp: u64,
    delay_time_period: u64,
    delay_block_period: u64,
) -> Result<(), StdError> {
    let processed_time = get_processed_time(deps.storage, EMPTY_PREFIX, &proof_height)?;
    let valid_time = processed_time + delay_time_period;

    if current_timestamp < valid_time {
        return Err(StdError::generic_err(format!(
            "cannot verify packet until time: {}, current time: {}",
            valid_time, current_timestamp
        )));
    }

    let processed_height: Height = get_processed_height(deps.storage, EMPTY_PREFIX, &proof_height)?;
    let valid_height = Height {
        revision_number: processed_height.revision_number,
        revision_height: processed_height.revision_height + delay_block_period,
    };

    let current_height = get_self_height(current_height);
    if current_height < valid_height {
        return Err(StdError::generic_err(format!(
            "cannot verify packet until height: {:?}, current height: {:?}",
            valid_height, current_height
        )));
    }

    Ok(())
}

/*
fn construct_upgrade_merkle_path(
upgrade_path: &Vec<String>,
client_upgrade_path: ibc::ics24_host::ClientUpgradePath,
) -> MerklePath {
let appended_key = ibc::ics24_host::Path::Upgrade(client_upgrade_path).to_string();

let mut result: Vec<String> = upgrade_path.clone();
result.push(appended_key);

MerklePath { key_path: result }
}
*/

fn is_expired(
    current_timestamp: u64,
    latest_timestamp: u64,
    light_client_state: &LightClientState,
) -> bool {
    current_timestamp > latest_timestamp + light_client_state.trusting_period
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crate::contract::types::ibc::MerklePrefix;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    /*
    use ics23::{
    calculate_existence_root, Proof, CommitmentProof, ExistenceProof, HashOp, InnerOp, LeafOp,
    LengthOp,
    };
    */

    /*
           #[test]
           fn test_verify_client_consensus_state() {
           let mut deps = mock_dependencies(&[]);
           let env = mock_env();

           let client_state = get_example_client_state(0, 5);

           let height = new_height(0, 5);
           let consensus_height = new_height(0, 5);

           let commitment_prefix = MerklePrefix {
           key_prefix: base64::encode("prefix"),
           };
           let counterparty_client_identifier = String::from("07-tendermint-0");

    // The counterparty_consensus_state + commitment_proof comes from the other side of the
    // bridge, while root "is local" (comes from proving consensus state).
    //
    // In the unittest we update provingConsensusState with "remote root", so that validation
    // always succeeds (as long as verify_membership works properly)
    let counterparty_consensus_state = CosmosConsensusState {
    root: MerkleRoot {
    hash: String::from("base64_encoded_hash"),
    },
    };

    let (commitment_proof, root) = get_example_proof(
    b"clients/07-tendermint-0/consensusStates/0-5".to_vec(), // key (based on consensus_height)
    to_vec(&counterparty_consensus_state).unwrap(),          // value
    );

    let proving_consensus_state = get_example_consenus_state(root, height);

    let response = verify_client_consensus_state(
    deps.as_mut(),
    env,
    client_state,
    height,
    consensus_height,
    commitment_prefix,
    counterparty_client_identifier,
    base64::encode(to_vec(&commitment_proof).unwrap()),
    counterparty_consensus_state,
    proving_consensus_state,
    );

    assert_eq!(response.is_err(), false);
    }
    */

    fn get_example_client_state(revision_number: u64, revision_height: u64) -> ClientState {
        let mut cl = ClientState::default();
        cl.latest_height = Height {
            revision_number,
            revision_height,
        };
        cl
    }

    fn get_example_consenus_state(root: String, height: Height) -> ConsensusState {
        // In real life scenario this consensus state would be fetched
        let mut light_cs = LightConsensusState::default();
        light_cs.number = height.revision_height + height.revision_number;
        ConsensusState::new(&light_cs, String::default(), 123, MerkleRoot { hash: root })
    }

    /*
    fn get_example_proof(key: Vec<u8>, value: Vec<u8>) -> (MerkleProof, Vec<u8>) {
    let leaf = LeafOp {
    hash: HashOp::Sha256.into(),
    prehash_key: 0,
    prehash_value: HashOp::Sha256.into(),
    length: LengthOp::VarProto.into(),
    prefix: vec![0_u8],
    };

    let valid_inner = InnerOp {
    hash: HashOp::Sha256.into(),
    prefix: hex::decode("deadbeef00cafe00").unwrap(),
    suffix: vec![],
    };

    let proof = ExistenceProof {
    key,
    value,
    leaf: Some(leaf.clone()),
    path: vec![valid_inner.clone()],
    };

    let root = calculate_existence_root(&proof).unwrap();
    let commitment_proof = CommitmentProof {
    proof: Some(Proof::Exist(proof)),
    };

    (
    MerkleProof {
    proofs: vec![commitment_proof],
    },
    root,
    )
    }
    */

    fn new_height(revision_number: u64, revision_height: u64) -> Height {
        Height {
            revision_number,
            revision_height,
        }
    }
}
