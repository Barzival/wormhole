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

pub mod byte_utils;
pub mod state;

use crate::byte_utils::{get_string_from_32, ByteUtils};

// near_sdk::setup_alloc!();

const CHAIN_ID_NEAR: u16 = 15;
const CHAIN_ID_SOL: u16 = 1;

const BRIDGE_TOKEN_BINARY: &'static [u8] =
    include_bytes!("../../ft/target/wasm32-unknown-unknown/release/ft.wasm");

/// Initial balance for the BridgeToken contract to cover storage and related.
const BRIDGE_TOKEN_INIT_BALANCE: Balance = 6_200_000_000_000_000_000_000;

/// Gas to initialize BridgeToken contract.
const BRIDGE_TOKEN_NEW: Gas = Gas(100_000_000_000_000);

/// Gas to call mint method on bridge token.
const MINT_GAS: Gas = Gas(10_000_000_000_000);

const NO_DEPOSIT: Balance = 0;

#[ext_contract(ext_ft_contract)]
pub trait FtContract {
    fn new(metadata: FungibleTokenMetadata, asset_meta: Vec<u8>, seq_number: u64);
    fn update_ft(metadata: FungibleTokenMetadata, asset_meta: Vec<u8>, seq_number: u64);
    fn ft_transfer_call(
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128>;
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
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
    fn vaa_withdraw(
        &self,
        from: AccountId,
        amount: u128,
        receiver: String,
        chain: u16,
        fee: u128,
        payload: String,
    ) -> String;
}

#[ext_contract(ext_worm_hole)]
pub trait Wormhole {
    fn verify_vaa(&self, vaa: String) -> u32;
    fn publish_message(&self, data: String, nonce: u32, refund_to: AccountId) -> u64;
}

#[ext_contract(ext_portal)]
pub trait ExtPortal {
    fn finish_deploy(&self, tkey: Vec<u8>, data: Vec<u8>, us: &mut UnwindState);
    fn vaa_transfer_complete(&self);
}

#[derive(Serialize, Deserialize)]
pub struct UnwindState {
    amount: Balance,
    sender_id: AccountId,
}

impl UnwindState {
    pub fn new() -> Self {
        Self {
            amount: 0,
            sender_id: AccountId::new_unchecked("".to_string()),
        }
    }

    pub fn unwind(&mut self) {
        // This will envolve over time...

        if self.amount > 0 {
            // Lets attempt to send the money back on error
            Promise::new(self.sender_id.clone()).transfer(self.amount);
            self.amount = 0;
        }
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Portal {
    booted: bool,
    core: AccountId,
    dups: UnorderedSet<Vec<u8>>,
    good: UnorderedSet<Vec<u8>>,
    owner_pk: PublicKey,
    emitter_registration: LookupMap<u16, Vec<u8>>,
    hash_map: LookupMap<Vec<u8>, String>,
    tokens: LookupMap<Vec<u8>, Vec<u8>>,
}

impl Default for Portal {
    fn default() -> Self {
        Self {
            booted: false,
            core: AccountId::new_unchecked("".to_string()),
            dups: UnorderedSet::new(b"d".to_vec()),
            good: UnorderedSet::new(b"g".to_vec()),
            owner_pk: env::signer_account_pk(),
            emitter_registration: LookupMap::new(b"c".to_vec()),
            hash_map: LookupMap::new(b"h".to_vec()),
            tokens: LookupMap::new(b"t".to_vec()),
        }
    }
}

fn vaa_register_chain(storage: &mut Portal, vaa: state::ParsedVAA, us: &mut UnwindState) {
    let data: &[u8] = &vaa.payload;
    let target_chain = data.get_u16(33);
    let chain = data.get_u16(35);

    if (target_chain != CHAIN_ID_NEAR) && (target_chain != 0) {
        us.unwind();
        env::panic_str("InvalidRegisterChainChain");
    }

    if storage.emitter_registration.contains_key(&chain) {
        us.unwind();
        env::panic_str("DuplicateChainRegistration");
    }

    storage
        .emitter_registration
        .insert(&chain, &data[37..69].to_vec());

    env::log_str(&format!(
        "register chain {} to {}",
        chain,
        hex::encode(&data[37..69])
    ));
}

fn vaa_upgrade_contract(storage: &mut Portal, vaa: state::ParsedVAA, us: &mut UnwindState) {
    let data: &[u8] = &vaa.payload;
    let chain = data.get_u16(33);
    if chain != CHAIN_ID_NEAR {
        us.unwind();
        env::panic_str("InvalidContractUpgradeChain");
    }

    us.unwind();
    env::panic_str("ContractUpgradesNotImplemented");
}

fn vaa_governance(storage: &mut Portal, vaa: state::ParsedVAA, us: &mut UnwindState, gov_idx: u32) {
    if gov_idx != vaa.guardian_set_index {
        us.unwind();
        env::panic_str("InvalidGovernanceSet");
    }

    if (CHAIN_ID_SOL != vaa.emitter_chain)
        || (hex::decode("0000000000000000000000000000000000000000000000000000000000000004")
            .unwrap()
            != vaa.emitter_address)
    {
        us.unwind();
        env::panic_str("InvalidGovernanceEmitter");
    }

    let data: &[u8] = &vaa.payload;
    let action = data.get_u8(32);

    match action {
        1u8 => vaa_register_chain(storage, vaa, us),
        2u8 => vaa_upgrade_contract(storage, vaa, us),
        _ => {
            us.unwind();
            env::panic_str("InvalidGovernanceAction")
        }
    }
}

fn vaa_transfer(storage: &mut Portal, vaa: state::ParsedVAA, us: &mut UnwindState) -> Promise {
    env::log_str(&hex::encode(&vaa.payload));

    let data: &[u8] = &vaa.payload[1..];

    let mut off = 32;
    while off < 64 && data[off] == 0 {
        off = off + 1;
    }
    let tkey = data[off..66].to_vec();

    let amount = data.get_u256(0);
    let token_address = data.get_bytes32(32).to_vec();
    let token_chain = data.get_u16(64);
    let recipient = data.get_bytes32(66).to_vec();
    let recipient_chain = data.get_u16(98);
    let fee = data.get_u256(100);

    if recipient_chain != CHAIN_ID_NEAR {
        us.unwind();
        env::panic_str("InvalidRecipientChain");
    }

    let reference = hex::encode(&tkey);
    let bridge_token_account = format!("{}.{}", reference, env::current_account_id());
    env::log_str(&bridge_token_account);

    if !env::is_valid_account_id(bridge_token_account.as_bytes()) {
        us.unwind();
        env::panic_str("InvalidAccountId");
    }

    if !storage.tokens.contains_key(&tkey) {
        env::panic_str("AssetNotAttested");
    }

    let bridge_token_account_id: AccountId = AccountId::new_unchecked(bridge_token_account.clone());

    return ext_ft_contract::ext(bridge_token_account_id)
        .with_attached_deposit(BRIDGE_TOKEN_INIT_BALANCE)
        .vaa_transfer(
            amount.1,
            token_address,
            token_chain,
            recipient,
            recipient_chain,
            fee.0,
        )
        .then(ext_portal::ext(env::current_account_id()).vaa_transfer_complete());
}

fn vaa_asset_meta(storage: &mut Portal, vaa: state::ParsedVAA, us: &mut UnwindState) -> Promise {
    env::log_str(&hex::encode(&vaa.payload));

    let data: &[u8] = &vaa.payload[1..];

    let mut off = 0;
    while off < 34 && data[off] == 0 {
        off = off + 1;
    }
    let tkey = data[off..34].to_vec();
    let token_chain = data.get_u16(32);
    let mut decimals = data.get_u8(34);
    let symbol = data.get_bytes32(35).to_vec();
    let name = data.get_bytes32(67).to_vec();

    if token_chain == CHAIN_ID_NEAR {
        us.unwind();
        env::panic_str("CannotAttestNearAssets");
    }

    let wname = get_string_from_32(&name) + " (Wormhole)";

    // Stick some useful meta-data into the asset to allow us to map backwards from a on-chain asset to the wormhole meta data
    let reference = hex::encode(&tkey);
    let ref_hash = env::sha256(&reference.as_bytes().to_vec());
    let bridge_token_account = format!("{}.{}", reference, env::current_account_id());
    env::log_str(&bridge_token_account);

    if !env::is_valid_account_id(bridge_token_account.as_bytes()) {
        us.unwind();
        env::panic_str("InvalidAccountId");
    }

    let bridge_token_account_id: AccountId = AccountId::new_unchecked(bridge_token_account.clone());

    // Decimals are capped at 8 in wormhole
    if decimals > 8 {
        decimals = 8;
    }

    let ft = FungibleTokenMetadata {
        spec: FT_METADATA_SPEC.to_string(),
        name: wname,
        symbol: get_string_from_32(&symbol),
        icon: Some("".to_string()), // Is there ANY way to supply this?
        reference: Some(reference.clone()),
        reference_hash: Some(Base64VecU8::from(ref_hash)),
        decimals: decimals,
    };

    if storage.tokens.contains_key(&tkey) {
        return ext_ft_contract::ext(bridge_token_account_id).update_ft(
            ft,
            data.to_vec(),
            vaa.sequence,
        );
    } else {
        let v = BRIDGE_TOKEN_BINARY.to_vec();

        if env::attached_deposit()
            < BRIDGE_TOKEN_INIT_BALANCE + (v.len() as u128 * env::storage_byte_cost())
        {
            us.unwind();
            env::panic_str("Not enough attached deposit to complete bridge token creation");
        }

        return Promise::new(bridge_token_account_id.clone())
            .create_account()
            .transfer(BRIDGE_TOKEN_INIT_BALANCE + (v.len() as u128 * env::storage_byte_cost()))
            .add_full_access_key(storage.owner_pk.clone())
            .deploy_contract(v)
            // Lets initialize it with useful stuff
            .then(ext_ft_contract::ext(bridge_token_account_id).new(
                ft,
                data.to_vec(),
                vaa.sequence,
            ))
            // And then lets tell us we are done!
            .then(ext_portal::ext(env::current_account_id()).finish_deploy(
                tkey,
                data.to_vec(),
                us,
            ));
    }
}

fn vaa_transfer_with_payload(
    storage: &mut Portal,
    vaa: state::ParsedVAA,
    us: &mut UnwindState,
) -> Promise {
    let data: &[u8] = &vaa.payload[1..];
    let amount = data.get_u256(0);
    let token_address = data.get_bytes32(32).to_vec();
    let token_chain = data.get_u16(64);
    let recipient = data.get_bytes32(66).to_vec();
    let recipient_chain = data.get_u16(98);
    let fee = data.get_u256(100);
    let payload = &data[132..];

    us.unwind();
    env::panic_str("vaa_transfer_with_payload");
}

pub fn submit_vaa_callback_work(
    storage: &mut Portal,
    vaa: String,
    us: &mut UnwindState,
    gov_idx: u32,
) -> Promise {
    //    env::log_str(&"submit_vaa_callback_work::top");

    let h = hex::decode(vaa).expect("invalidVaa");

    let pvaa = state::ParsedVAA::parse(&h);

    if pvaa.version != 1 {
        us.unwind();
        env::panic_str("InvalidVersion");
    }

    let data: &[u8] = &pvaa.payload;

    let governance = data[0..32]
        == hex::decode("000000000000000000000000000000000000000000546f6b656e427269646765").unwrap();
    let action = data.get_u8(0);

    if !governance && action == 2u8 {
        if storage.good.contains(&pvaa.hash) {
            //            env::log_str(&"submit_vaa_callback_work::remove");
            storage.good.remove(&pvaa.hash);
        } else {
            //            env::log_str(&"submit_vaa_callback_work::insert and return");
            storage.good.insert(&pvaa.hash);
            return Promise::new(env::signer_account_id());
        }
    }

    //    env::log_str(&"submit_vaa_callback_work::getting_work_done");

    // Check if VAA with this hash was already accepted
    if storage.dups.contains(&pvaa.hash) {
        us.unwind();
        env::panic_str("alreadyExecuted");
    }
    storage.dups.insert(&pvaa.hash);

    if governance {
        vaa_governance(storage, pvaa, us, gov_idx);
        return Promise::new(env::signer_account_id());
    }

    //    env::log_str(&format!("looking up chain {}", pvaa.emitter_chain));

    if !storage
        .emitter_registration
        .contains_key(&pvaa.emitter_chain)
    {
        us.unwind();
        env::panic_str("ChainNotRegistered");
    }

    if storage
        .emitter_registration
        .get(&pvaa.emitter_chain)
        .unwrap()
        != pvaa.emitter_address
    {
        us.unwind();
        env::panic_str("InvalidRegistration");
    }

    //    env::log_str(&"submit_vaa_callback_work::branching_to_actions");

    return match action {
        1u8 => vaa_transfer(storage, pvaa, us),
        2u8 => vaa_asset_meta(storage, pvaa, us),
        3u8 => vaa_transfer_with_payload(storage, pvaa, us),
        _ => {
            us.unwind();
            env::panic_str("InvalidPortalAction");
        }
    };
}

#[near_bindgen]
impl Portal {
    #[private]
    #[payable]
    pub fn emitter_callback(
        &mut self,
        refund_to: AccountId,
        dust: u128, // transfer near on success
        #[callback_result] seq: Result<u64, PromiseError>,
    ) -> u64 {
        if seq.is_err() {
            if env::attached_deposit() != 0 {
                Promise::new(refund_to).transfer(env::attached_deposit()); // transfer back on failure
            }
            env::panic_str("EmitFail");
        }

        if dust != 0 {
            Promise::new(refund_to).transfer(dust);
        }

        return seq.unwrap();
    }

    #[payable]
    pub fn send_transfer_near(
        &mut self,
        receiver: String,
        chain: u16,
        fee: u128,
        payload: String,
    ) -> Promise {
        let amount = env::attached_deposit();
        if amount == 0 {
            env::panic_str("level up your game");
        }

        const NEAR_MULT: u128 = 10_000_000_000_000_000; // 1e16

        let namount = amount / NEAR_MULT;
        let nfee = fee / NEAR_MULT;
        let dust = amount - (namount * NEAR_MULT) - (nfee * NEAR_MULT);

        let mut p = [
            // PayloadID uint8 = 1
            (if payload.len() == 0 { 1 } else { 3 } as u8)
                .to_be_bytes()
                .to_vec(),
            // Amount uint256
            vec![0; 24],
            (namount as u64).to_be_bytes().to_vec(),
            //TokenAddress bytes32
            vec![0; 32],
            // TokenChain uint16
            (CHAIN_ID_NEAR as u16).to_be_bytes().to_vec(),
            // To bytes32
            vec![0; (64 - receiver.len()) / 2],
            hex::decode(receiver).unwrap(),
            // ToChain uint16
            (chain as u16).to_be_bytes().to_vec(),
        ]
        .concat();

        if payload.len() == 0 {
            p = [p, vec![0; 24], (nfee as u64).to_be_bytes().to_vec()].concat();
            if p.len() != 133 {
                Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit());
                env::panic_str(&format!("paylod1 formatting errro  len = {}", p.len()));
            }
        } else {
            p = [p, hex::decode(&payload).unwrap()].concat();
            if p.len() != (133 + (payload.len() / 2)) {
                Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit());
                env::panic_str(&format!("paylod3 formatting errro  len = {}", p.len()));
            }
        }

        ext_worm_hole::ext(self.core.clone())
            .publish_message(
                hex::encode(p),
                env::block_height() as u32,
                env::current_account_id(),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_attached_deposit(env::attached_deposit())
                    .emitter_callback(env::predecessor_account_id(), dust),
            )
    }

    pub fn account_hash(&mut self) -> (String, String) {
        let acct = env::current_account_id().clone();
        let astr = acct.to_string();

        return (astr.clone(), hex::encode(env::sha256(&astr.as_bytes())));
    }

    pub fn is_wormhole(&self, token: &String) -> bool {
        let acct = env::current_account_id().clone();
        let account = acct.as_str();
        let a = &token[(token.len() - &account.len())..];

        env::log_str(&format!("a {} account {} token {}", &a, &account, &token));

        return a == account;
    }

    #[payable]
    pub fn send_transfer_token(
        &mut self,
        amount: u128,
        token: String,
        receiver: String,
        chain: u16,
        fee: u128,
        payload: String,
    ) -> Promise {
        if self.is_wormhole(&token) {
            ext_ft_contract::ext(AccountId::try_from(token.clone()).unwrap())
                .vaa_withdraw(
                    env::predecessor_account_id(),
                    amount,
                    receiver,
                    chain,
                    fee,
                    payload,
                )
                .then(
                    Self::ext(env::current_account_id())
                        .with_attached_deposit(env::attached_deposit())
                        .send_transfer_token_wormhole_callback(env::predecessor_account_id()),
                )
        } else {
            env::panic_str("NotWormhole");
        }
    }

    #[private]
    #[payable]
    pub fn send_transfer_token_wormhole_callback(
        &mut self,
        refund_to: AccountId,
        #[callback_result] payload: Result<String, PromiseError>,
    )  -> Promise  {
        if payload.is_err() {
            env::panic_str("PayloadError");
        }

        // Failing here would suck chunks...  The money has already
        // been burned from the account so we better succeed...

        ext_worm_hole::ext(self.core.clone())
            .with_attached_deposit(env::attached_deposit())
            .publish_message(
                payload.unwrap(),
                env::block_height() as u32,
                refund_to.clone(),
            )
            .then(
                Self::ext(env::current_account_id())
                    .emitter_callback(refund_to, 0),
            )
    }

    #[private]
    #[payable]
    pub fn send_transfer_token_native_callback(
        &mut self,
        token: String,
        receiver: String,
        chain: u16,
        fee: u128,
        payload: String,
        refund_to: AccountId,
        #[callback_result] ft_info: Result<FungibleTokenMetadata, PromiseError>,
    ) {
        if env::predecessor_account_id() != self.core {
            env::panic_str("Odd native");
        }
        env::panic_str("native callback");

        //        if ft_info.is_err() {
        //            if env::attached_deposit() != 0 {
        //                Promise::new(env::signer_account_id()).transfer(env::attached_deposit());
        //            }
        //            env::panic_str("FailedToRetrieveMetaData");
        //        }
        //
        //        let ft = ft_info.unwrap();
        //        let tref = ft.reference;
        //        let mut wormhole = false;
        //        let mut tinfo = Vec::new();
        //
        //        if tref != None {
        //            let r = hex::decode(&tref.unwrap()).unwrap();
        //            if self.tokens.contains_key(&r) {
        //                wormhole = true;
        //
        //
        //            }
        //        }

        /*
                const NEAR_MULT: u128 = 10_000_000_000_000_000; // 1e16

                let namount = amount / NEAR_MULT;
                let nfee = fee / NEAR_MULT;
                let dust = amount - (namount * NEAR_MULT) - (nfee * NEAR_MULT);

                let mut p = [
                    // PayloadID uint8 = 1
                    (if payload.len() == 0 { 1 } else { 3 } as u8)
                        .to_be_bytes()
                        .to_vec(),
                    // Amount uint256
                    vec![0; 24],
                    (namount as u64).to_be_bytes().to_vec(),
                    //TokenAddress bytes32
                    vec![0; 32],
                    // TokenChain uint16
                    (CHAIN_ID_NEAR as u16).to_be_bytes().to_vec(),
                    // To bytes32
                    vec![0; (64 - receiver.len()) / 2],
                    hex::decode(receiver).unwrap(),
                    // ToChain uint16
                    (chain as u16).to_be_bytes().to_vec(),
                ]
                .concat();

                if payload.len() == 0 {
                    p = [p, vec![0; 24], (nfee as u64).to_be_bytes().to_vec()].concat();
                    if p.len() != 133 {
                        Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit());
                        env::panic_str(&format!("paylod1 formatting errro  len = {}", p.len()));
                    }
                } else {
                    p = [p, hex::decode(&payload).unwrap()].concat();
                    if p.len() != (133 + (payload.len() / 2)) {
                        Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit());
                        env::panic_str(&format!("paylod3 formatting errro  len = {}", p.len()));
                    }
                }

                ext_worm_hole::ext(self.core.clone())
                    .publish_message(
                        hex::encode(p),
                        env::block_height() as u32,
                        env::current_account_id(),
                    )
                    .then(
                        Self::ext(env::current_account_id())
                            .with_attached_deposit(env::attached_deposit())
                            .emitter_callback(env::predecessor_account_id(), dust),
                    )
        */
    }

    #[payable]
    pub fn submit_vaa(&mut self, vaa: String) -> Promise {
        //        env::log_str(&"submit_vaa::start");
        let mut us: UnwindState = UnwindState::new();

        let h = hex::decode(&vaa).expect("invalidVaa");

        // Please optimize this next time you are bored and just have it do the hash calculation...
        let pvaa = state::ParsedVAA::parse(&h);

        us.sender_id = env::signer_account_id();

        if self.good.contains(&pvaa.hash) {
            return submit_vaa_callback_work(self, vaa, &mut us, 0);
        } else {
            us.amount = env::attached_deposit();

            return ext_worm_hole::ext(self.core.clone())
                .verify_vaa(vaa.clone())
                .then(Self::ext(env::current_account_id()).submit_vaa_callback(vaa, &mut us));
        }
    }

    #[payable]
    pub fn attest_near(&mut self) -> Promise {
        let p = [
            (2 as u8).to_be_bytes().to_vec(),
            vec![0; 32],
            (CHAIN_ID_NEAR as u16).to_be_bytes().to_vec(),
            (24 as u8).to_be_bytes().to_vec(), // yectoNEAR is 1e24 ...
            byte_utils::extend_string_to_32("NEAR"),
            byte_utils::extend_string_to_32("NEAR"),
        ]
        .concat();

        if p.len() != 100 {
            if env::attached_deposit() != 0 {
                Promise::new(env::predecessor_account_id()).transfer(env::attached_deposit());
            }
            env::log_str(&format!("len: {}  val: {}", p.len(), hex::encode(p)));
            env::panic_str("Formatting error");
        }

        ext_worm_hole::ext(self.core.clone())
            .with_attached_deposit(env::attached_deposit())
            .publish_message(
                hex::encode(p),
                env::block_height() as u32,
                env::predecessor_account_id(),
            )
            .then(
                Self::ext(env::current_account_id())
                    .emitter_callback(env::predecessor_account_id(), 0),
            )
    }

    #[payable]
    pub fn attest_token(&mut self, token: String) -> Promise {
        ext_ft_contract::ext(AccountId::try_from(token.clone()).unwrap())
            .ft_metadata()
            .then(
                Self::ext(env::current_account_id())
                    .with_attached_deposit(env::attached_deposit())
                    .attest_token_callback(token, env::predecessor_account_id()),
            )
    }

    #[private]
    pub fn attest_token_callback(
        &mut self,
        token: String,
        refund_to: AccountId,
        #[callback_result] ft_info: Result<FungibleTokenMetadata, PromiseError>,
    ) -> Promise {
        if ft_info.is_err() {
            if env::attached_deposit() != 0 {
                Promise::new(env::signer_account_id()).transfer(env::attached_deposit());
            }
            env::panic_str("FailedToRetrieveMetaData");
        }

        let ft = ft_info.unwrap();
        let tref = ft.reference;

        let mut p = Vec::new();

        if tref != None {
            let r = hex::decode(&tref.unwrap()).unwrap();
            if self.tokens.contains_key(&r) {
                p = [
                    (2 as u8).to_be_bytes().to_vec(),
                    self.tokens.get(&r).unwrap(),
                ]
                .concat();
            }
        }

        if p.len() == 0 {
            let h = env::sha256(&token.as_bytes());
            // We should charge for this....
            self.hash_map.insert(&h, &token);
            p = [
                (2 as u8).to_be_bytes().to_vec(),
                h,
                (CHAIN_ID_NEAR as u16).to_be_bytes().to_vec(),
                (ft.decimals as u8).to_be_bytes().to_vec(), // yectoNEAR is 1e24 ...
                byte_utils::extend_string_to_32(&ft.symbol),
                byte_utils::extend_string_to_32(&ft.name),
            ]
            .concat();
        }

        if p.len() != 100 {
            env::log_str(&format!("len: {}  val: {}", p.len(), hex::encode(p)));
            env::panic_str("Formatting error");
        }

        ext_worm_hole::ext(self.core.clone())
            .with_attached_deposit(env::attached_deposit())
            .publish_message(hex::encode(p), env::block_height() as u32, refund_to)
            .then(
                Self::ext(env::current_account_id())
                    .emitter_callback(env::predecessor_account_id(), 0),
            )
    }

    #[private]
    pub fn vaa_transfer_complete(&mut self) {
        if !is_promise_success() {
            env::log_str(&"is_promise_success() = false");
            unsafe {
                sys::panic();
            }
        } else {
            env::log_str(&"is_promise_success() = true");
        }
    }

    #[private]
    pub fn finish_deploy(&mut self, tkey: Vec<u8>, data: Vec<u8>, us: &mut UnwindState) {
        if is_promise_success() {
            env::log_str(&"We made it... what does that mean?");
            env::log_str(&hex::encode(&tkey));
            self.tokens.insert(&tkey, &data);
        } else {
            //us.unwind(); // what does this mean?
            env::panic_str("bad deploy");
        }
    }

    #[private] // So, all of wormhole security rests in this one statement?
    pub fn submit_vaa_callback(
        &mut self,
        vaa: String,
        us: &mut UnwindState,
        #[callback_result] gov_idx: Result<u32, PromiseError>,
    ) {
        // Is this even needed anymore?
        if (env::promise_results_count() != 1)
            || (env::predecessor_account_id() != env::current_account_id())
        {
            us.unwind();
            env::panic_str("BadPredecessorAccount");
        }

        // Is there anyway to confirm the person I called and is
        // calling me back here is the person is the core_contract?

        if gov_idx.is_err() {
            us.unwind();
            env::panic_str("vaaVerifyFail");
        }

        submit_vaa_callback_work(self, vaa, us, gov_idx.unwrap()).as_return();
    }

    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        env::panic_str("ft_on_transfer");
        return PromiseOrValue::Value(U128::from(0));
    }

    pub fn boot_portal(&mut self, core: String) {
        if self.owner_pk != env::signer_account_pk() {
            env::panic_str("invalidSigner");
        }

        if self.booted {
            env::panic_str("NoDonut");
        }
        self.booted = true;
        self.core = AccountId::try_from(core.clone()).unwrap();
    }
}
