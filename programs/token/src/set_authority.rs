use lez_authority::Authority;
use nssa_core::{
    account::{AccountId, AccountWithMetadata, Data},
    program::AccountPostState,
};
use token_core::TokenDefinition;

/// Rotate or revoke the mint authority on a fungible token definition.
///
/// Uses the `lez-authority` crate (RFP-001) for standardised access control.
///
/// - `new_authority: Some(id)` — transfers authority to `id`. Previous authority
///   loses all minting rights immediately.
/// - `new_authority: None` — permanently renounces authority. Supply is fixed
///   from this point on. This operation is irreversible.
///
/// Required accounts:
/// 1. Token Definition account (initialized, authorized by current mint authority).
pub fn set_authority(
    definition_account: AccountWithMetadata,
    new_authority: Option<AccountId>,
) -> Vec<AccountPostState> {
    let mut definition = TokenDefinition::try_from(&definition_account.account.data)
        .expect("Definition account must be valid");

    match &mut definition {
        TokenDefinition::Fungible { mint_authority, .. } => {
            let mut auth = Authority::from_option(*mint_authority);

            match new_authority {
                Some(new_id) => {
                    auth.rotate(new_id, definition_account.is_authorized)
                        .unwrap_or_else(|e| panic!("{e}"));
                }
                None => {
                    auth.revoke(definition_account.is_authorized)
                        .unwrap_or_else(|e| panic!("{e}"));
                }
            }

            *mint_authority = auth.into_option();
        }
        TokenDefinition::NonFungible { .. } => {
            panic!("Cannot set mint authority on a Non-Fungible Token definition");
        }
    }

    let mut definition_post = definition_account.account;
    definition_post.data = Data::from(&definition);

    vec![AccountPostState::new(definition_post)]
}
