use lez_authority::Authority;
use nssa_core::{
    account::{Account, AccountWithMetadata, Data},
    program::{AccountPostState, Claim, ProgramId},
};
use token_core::{TokenDefinition, TokenHolding};

/// Mint new tokens to the holder's account.
///
/// Uses the `lez-authority` crate (RFP-001) to enforce that only the current
/// mint authority can mint additional supply.
///
/// Required accounts:
/// 1. Token Definition account (initialized, authorized by mint authority).
/// 2. Token Holding account (initialized, or uninitialized with holder authorization).
pub fn mint(
    definition_account: AccountWithMetadata,
    user_holding_account: AccountWithMetadata,
    amount_to_mint: u128,
    token_program_id: ProgramId,
) -> Vec<AccountPostState> {
    assert_eq!(
        definition_account.account.program_owner, token_program_id,
        "Token definition must be owned by token program"
    );

    let mut definition = TokenDefinition::try_from(&definition_account.account.data)
        .expect("Definition account must be valid");

    // Enforce mint authority via lez-authority (RFP-001)
    match &definition {
        TokenDefinition::Fungible { mint_authority, .. } => {
            let auth = Authority::from_option(*mint_authority);
            auth.require(definition_account.is_authorized)
                .unwrap_or_else(|e| panic!("{e}"));
        }
        TokenDefinition::NonFungible { .. } => {
            panic!("Cannot mint additional supply for Non-Fungible Tokens");
        }
    }

    let mut holding = if user_holding_account.account == Account::default() {
        TokenHolding::zeroized_from_definition(definition_account.account_id, &definition)
    } else {
        TokenHolding::try_from(&user_holding_account.account.data)
            .expect("Token Holding account must be valid")
    };

    assert_eq!(
        definition_account.account_id,
        holding.definition_id(),
        "Mismatch Token Definition and Token Holding"
    );

    match (&mut definition, &mut holding) {
        (
            TokenDefinition::Fungible { total_supply, .. },
            TokenHolding::Fungible { balance, .. },
        ) => {
            *balance = balance
                .checked_add(amount_to_mint)
                .expect("Balance overflow on minting");
            *total_supply = total_supply
                .checked_add(amount_to_mint)
                .expect("Total supply overflow");
        }
        _ => panic!("Mismatched Token Definition and Token Holding types"),
    }

    let mut definition_post = definition_account.account;
    definition_post.data = Data::from(&definition);

    let mut holding_post = user_holding_account.account;
    holding_post.data = Data::from(&holding);

    vec![
        AccountPostState::new(definition_post),
        AccountPostState::new_claimed_if_default(holding_post, Claim::Authorized),
    ]
}
