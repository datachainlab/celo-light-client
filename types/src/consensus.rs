use crate::errors::Error;
use crate::istanbul::ValidatorData;
use ethereum_types::H256;
use rlp_derive::{RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};

/// LightConsensusState represents an IBFT consensus state at specified block height
#[derive(Serialize, Deserialize, RlpDecodable, RlpEncodable, Clone, PartialEq, Debug, Default)]
pub struct LightConsensusState {
    /// Block number at which the snapshot was created
    pub number: u64,
    /// Snapshot of current validator set
    pub validators: Vec<ValidatorData>,
    // Hash and aggregated seal are required to validate the header against the validator set
    /// Block H256
    pub hash: H256,
}

impl LightConsensusState {
    pub fn verify(&self) -> Result<(), Error> {
        //TODO!!!
        Ok(())
    }
}

#[cfg(feature = "web3_support")]
impl From<web3::types::Snapshot<ValidatorData>> for LightConsensusState {
    fn from(snap: web3::types::Snapshot<ValidatorData>) -> Self {
        Self {
            number: snap.number,
            validators: snap.validators,
            hash: snap.hash,
        }
    }
}
