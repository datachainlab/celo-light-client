use crate::istanbul::istanbul_filtered_header;
use crate::types::istanbul::ISTANBUL_EXTRA_VANITY_LENGTH;
use crate::traits::default::{DefaultFrom, FromBytes};
use crate::serialization::rlp::{rlp_field_from_bytes, rlp_to_big_int};
use crate::slice_as_array_ref;
use crate::errors::{Kind, Error};
use rug::{integer::Order, Integer};
use rlp::{DecoderError, Decodable, Rlp, Encodable, RlpStream};
use sha3::{Digest, Keccak256};

/// HASH_LENGTH represents the number of bytes used in a header hash
pub const HASH_LENGTH: usize = 32;

/// ADDRESS_LENGTH represents the number of bytes used in a header Ethereum account address
pub const ADDRESS_LENGTH: usize = 20;

/// BLOOM_BYTE_LENGTH represents the number of bytes used in a header log bloom
pub const BLOOM_BYTE_LENGTH: usize = 256;

/// Hash is the output of the cryptographic digest function
pub type Hash = [u8; HASH_LENGTH];

/// Address represents the 20 byte address of an Ethereum account
pub type Address = [u8; ADDRESS_LENGTH];

/// Bloom represents a 2048 bit bloom filter
pub type Bloom = [u8; BLOOM_BYTE_LENGTH];

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    #[serde(with = "crate::serialization::bytes::hexstring")]
    pub parent_hash: Hash,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "miner")]
    pub coinbase: Address,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "stateRoot")]
    pub root: Hash,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "transactionsRoot")]
    pub tx_hash: Hash,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "receiptsRoot")]
    pub receipt_hash: Hash,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "logsBloom")]
    pub bloom: Bloom,

    #[serde(with = "crate::serialization::bytes::hexbigint")]
    pub number: Integer,

    #[serde(with = "crate::serialization::bytes::hexnum")]
    pub gas_used: u64,

    #[serde(rename = "timestamp")]
    #[serde(with = "crate::serialization::bytes::hexnum")]
    pub time: u64,

    #[serde(with = "crate::serialization::bytes::hexstring")]
    #[serde(rename = "extraData")]
    pub extra: Vec<u8>
}

impl Header {
    pub fn new() -> Self {
        Self {
            parent_hash: Hash::default(),
            coinbase: Address::default(),
            root: Hash::default(),
            tx_hash: Hash::default(),
            receipt_hash: Hash::default(),
            bloom: Bloom::default(),
            number: Integer::default(),
            gas_used: u64::default(),
            time: u64::default(),
            extra: Vec::default(),
        }
    }

    pub fn from_rlp(bytes: &[u8]) -> Result<Self, Error>{
        match rlp::decode(&bytes) {
            Ok(header) => Ok(header),
            Err(err) => Err(Kind::RlpDecodeError.context(err).into()),
        }
    }

    pub fn to_rlp(&self) -> Vec<u8> {
        rlp::encode(self)
    }

    pub fn hash(&self) -> Result<Hash, Error> {
        if self.extra.len() >= ISTANBUL_EXTRA_VANITY_LENGTH {
            let istanbul_header = istanbul_filtered_header(&self, true);
            if istanbul_header.is_ok() {
                return rlp_hash(&istanbul_header.unwrap());
            }
        }

        rlp_hash(self)
    }
}

impl Encodable for Header {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(10);

        // parent_hash
        s.append(&self.parent_hash.to_vec());

        // coinbase
        s.append(&self.coinbase.to_vec()); // TODO: can we do it without conversion?

        // root
        s.append(&self.root.to_vec());

        // tx_hash
        s.append(&self.tx_hash.to_vec());

        // receipt_hash
        s.append(&self.receipt_hash.to_vec());

        // bloom
        s.append(&self.bloom.to_vec());

        // number
        s.append(&self.number.to_digits(Order::Msf));

        // gas_used
        s.append(&self.gas_used);

        // time
        s.append(&self.time);

        // extra
        s.append(&self.extra);
    }
}

impl Decodable for Header {
        fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
            Ok(Header{
                parent_hash: rlp_field_from_bytes(rlp, 0)?,
                coinbase: rlp_field_from_bytes(rlp, 1)?,
                root: rlp_field_from_bytes(rlp, 2)?,
                tx_hash: rlp_field_from_bytes(rlp, 3)?,
                receipt_hash: rlp_field_from_bytes(rlp, 4)?,
                bloom: rlp_field_from_bytes(rlp, 5)?,
                number: rlp_to_big_int(rlp, 6)?,
                gas_used: rlp.val_at(7)?,
                time: rlp.val_at(8)?,
                extra: rlp.val_at(9)?,
            })
        }
}

impl DefaultFrom for Bloom {
    fn default() -> Self {
        [0; BLOOM_BYTE_LENGTH]
    }
}

impl FromBytes for Bloom {
    fn from_bytes(data: &[u8]) -> Result<&Bloom, Error> {
        slice_as_array_ref!(
            &data[..BLOOM_BYTE_LENGTH],
            BLOOM_BYTE_LENGTH
        )
    }
}

impl FromBytes for Address {
    fn from_bytes(data: &[u8]) -> Result<&Address, Error> {
        slice_as_array_ref!(
            &data[..ADDRESS_LENGTH],
            ADDRESS_LENGTH
        )
    }
}

fn rlp_hash(header: &Header) -> Result<Hash, Error> {
    let digest = Keccak256::digest(&rlp::encode(header));

    Ok(slice_as_array_ref!(&digest[..HASH_LENGTH], HASH_LENGTH)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_WITH_EMPTY_EXTRA: &str = "f901a6a07285abd5b24742f184ad676e31f6054663b3529bc35ea2fcad8a3e0f642a46f7948888f1f195afa192cfee860698584c030f4c9db1a0ecc60e00b3fe5ce9f6e1a10e5469764daf51f1fe93c22ec3f9a7583a80357217a0d35d334d87c0cc0a202e3756bf81fae08b1575f286c7ee7a3f8df4f0f3afc55da056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421b901000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001825208845c47775c80";

    #[test]
    fn encodes_header_to_rlp() {
        let bytes = hex::decode(&HEADER_WITH_EMPTY_EXTRA).unwrap();
        let header = Header::from_rlp(&bytes).unwrap();
        let encoded_bytes = header.to_rlp();

        assert_eq!(encoded_bytes, bytes);
    }

    pub fn to_hash<T>(data: &str) -> T where T: FromBytes + Clone {
        T::from_bytes(&hex::decode(data).unwrap()).unwrap().to_owned()
    }

    #[test]
    fn decodes_header_from_rlp() {
        let expected = vec![
            Header {
                parent_hash: to_hash("7285abd5b24742f184ad676e31f6054663b3529bc35ea2fcad8a3e0f642a46f7"),
                coinbase: to_hash("8888f1f195afa192cfee860698584c030f4c9db1"),
                root: to_hash("ecc60e00b3fe5ce9f6e1a10e5469764daf51f1fe93c22ec3f9a7583a80357217"),
                tx_hash: to_hash("d35d334d87c0cc0a202e3756bf81fae08b1575f286c7ee7a3f8df4f0f3afc55d"),
                receipt_hash: to_hash("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"),
                bloom: Bloom::default(),
                number: Integer::from(1),
                gas_used: 0x5208,
                time: 0x5c47775c,
                extra: Vec::default(),
            },
        ];

        for (bytes, expected_ist) in vec![
            hex::decode(&HEADER_WITH_EMPTY_EXTRA).unwrap(),
        ].iter().zip(expected) {
            let parsed = Header::from_rlp(&bytes).unwrap();

            assert_eq!(parsed, expected_ist);
        }
    }

    #[test]
    fn serializes_and_deserializes_to_json() {
        for bytes in vec![
            hex::decode(&HEADER_WITH_EMPTY_EXTRA).unwrap(),
        ].iter() {
            let parsed = Header::from_rlp(&bytes).unwrap();
            let json_string = serde_json::to_string(&parsed).unwrap();
            let deserialized_from_json: Header = serde_json::from_str(&json_string).unwrap();

            assert_eq!(parsed, deserialized_from_json);
        }
    }
}