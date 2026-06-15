use nssa_core::{
    account::{AccountId, AccountWithMetadata, Data},
    program::AccountPostState,
};
use token_core::TokenDefinition;

pub fn set_authority(
    definition_account: AccountWithMetadata,
    new_authority: Option<AccountId>,
) -> Vec<AccountPostState> {
    assert!(
        definition_account.is_authorized,
        "Definition account must be authorized by current mint authority"
    );

    let mut definition = TokenDefinition::try_from(&definition_account.account.data)
        .expect("Definition account must be valid");

    match &mut definition {
        TokenDefinition::Fungible { mint_authority, .. } => {
            assert!(
                mint_authority.is_some(),
                "Mint authority is already revoked; cannot rotate a revoked authority"
            );
            *mint_authority = new_authority;
        }
        TokenDefinition::NonFungible { .. } => {
            panic!("Cannot set mint authority on a Non-Fungible Token definition");
        }
    }

    let mut definition_post = definition_account.account;
    definition_post.data = Data::from(&definition);

    vec![AccountPostState::new(definition_post)]
}
