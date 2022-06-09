use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider,
};

use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::collections::LazyOption;
use near_sdk::json_types::{U128};

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{
    env, near_bindgen, AccountId, PanicOnDefault,
    PromiseOrValue, StorageUsage,
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MochFTContract {
    token: FungibleToken,
    meta: LazyOption<FungibleTokenMetadata>,
    controller: AccountId
}

#[near_bindgen]
impl MochFTContract {
    #[init]
    pub fn new(metadata: FungibleTokenMetadata) -> Self {
        assert!(!env::state_exists(), "Already initialized");

        metadata.assert_valid();

        Self {
            token: FungibleToken::new(b"ft".to_vec()),
            meta: LazyOption::new(b"md".to_vec(), Some(&metadata)),
            controller: env::predecessor_account_id(),
        }
    }

    pub fn account_storage_usage(&self) -> StorageUsage {
        self.token.account_storage_usage
    }
}

near_contract_standards::impl_fungible_token_core!(MochFTContract, token);
near_contract_standards::impl_fungible_token_storage!(MochFTContract, token);

#[near_bindgen]
impl FungibleTokenMetadataProvider for MochFTContract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.meta.get().unwrap()
    }
}
