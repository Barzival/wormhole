//#![allow(unused_mut)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::collections::LazyOption;
use near_sdk::json_types::{Base64VecU8, U128};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, Balance, Gas, PanicOnDefault, Promise,
    PromiseError, PromiseOrValue, PromiseResult, PublicKey,
};
use serde::{de, Deserialize, Serialize};

use near_sdk::serde_json::Value;

use near_sdk::utils::is_promise_success;

use near_sys as sys;

use hex;

const CHAIN_ID_NEAR: u16 = 15;
const CHAIN_ID_SOL: u16 = 1;

const BRIDGE_TOKEN_BINARY: &'static [u8] =
    include_bytes!("../../ft/target/wasm32-unknown-unknown/release/ft.wasm");

/// Initial balance for the BridgeToken contract to cover storage and related.
const BRIDGE_TOKEN_INIT_BALANCE: Balance = 5_860_000_000_000_000_000_000;

/// Gas to initialize BridgeToken contract.
const BRIDGE_TOKEN_NEW: Gas = Gas(100_000_000_000_000);

/// Gas to call mint method on bridge token.
const MINT_GAS: Gas = Gas(10_000_000_000_000);

const NO_DEPOSIT: Balance = 0;

#[ext_contract(ext_ft_contract)]
pub trait FtContract {
    fn new(metadata: FungibleTokenMetadata, asset_meta: Vec<u8>, seq_number: u64);
    fn update_ft(&self, metadata: FungibleTokenMetadata, asset_meta: Vec<u8>, seq_number: u64);
    fn ft_metadata(&self) -> FungibleTokenMetadata;
    fn vaa_transfer(
        &self,
        amount: u128,
        token_address: Vec<u8>,
        token_chain: u16,
        recipient: Vec<u8>,
        recipient_chain: u16,
        fee: u128,
    );
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct PortalTest {}

impl Default for PortalTest {
    fn default() -> Self {
        Self {}
    }
}

#[near_bindgen]
impl PortalTest {
    pub fn deploy_ft(&mut self) -> Promise {
        let name = format!("b{}", env::block_height());

        let bridge_token_account = format!("{}.{}", name, env::current_account_id());

        let bridge_token_account_id: AccountId =
            AccountId::new_unchecked(bridge_token_account.clone());

        let ft = FungibleTokenMetadata {
            spec: FT_METADATA_SPEC.to_string(),
            name: name.clone(),
            symbol: name,
            icon: Some("".to_string()), // Is there ANY way to supply this?
            reference: None,
            reference_hash: None,
            decimals: 15,
        };

        let v = BRIDGE_TOKEN_BINARY.to_vec();

        Promise::new(bridge_token_account_id.clone())
            .create_account()
            .transfer(BRIDGE_TOKEN_INIT_BALANCE + (v.len() as u128 * env::storage_byte_cost()))
            .add_full_access_key(env::signer_account_pk())
            .deploy_contract(v)
            // Lets initialize it with useful stuff
            .then(ext_ft_contract::ext(bridge_token_account_id).new(
                ft,
                b"".to_vec(),
                env::block_height(),
            ))
            // And then lets tell us we are done!
            .then(Self::ext(env::current_account_id()).finish_deploy(bridge_token_account))
    }

    #[private]
    pub fn finish_deploy(&mut self, ret: String) -> String {
        if is_promise_success() {
            return ret;
        } else {
            env::panic_str("bad deploy");
        }
    }
}
