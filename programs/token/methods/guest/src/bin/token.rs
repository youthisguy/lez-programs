#![cfg_attr(not(test), no_main)]

use spel_framework::prelude::*;
use spel_framework::context::ProgramContext;
use nssa_core::account::AccountWithMetadata;

#[cfg(not(test))]
risc0_zkvm::guest::entry!(main);

#[lez_program(instruction = "token_core::Instruction")]
mod token {
    #[expect(
        unused_imports,
        reason = "SPEL instruction macro requires importing parent-scope handler types"
    )]
    use super::*;

    /// Transfer tokens from sender to recipient.
    /// Fresh public recipients must be explicitly authorized in the same transaction.
    #[instruction]
    pub fn transfer(
        sender: AccountWithMetadata,
        recipient: AccountWithMetadata,
        amount_to_transfer: u128,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(token_program::transfer::transfer(
            sender,
            recipient,
            amount_to_transfer,
        ), vec![]))
    }

    /// Create a new fungible token definition without metadata.
    /// Supply is fixed — no mint authority is set.
    #[instruction]
    pub fn new_fungible_definition(
        definition_target_account: AccountWithMetadata,
        holding_target_account: AccountWithMetadata,
        name: String,
        total_supply: u128,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(
            token_program::new_definition::new_fungible_definition(
                definition_target_account,
                holding_target_account,
                name,
                total_supply,
            ),
            vec![],
        ))
    }

    /// Create a new fungible token definition with an optional mint authority.
    /// `mint_authority: Some(id)` enables future minting; `None` fixes supply immediately.
    #[instruction]
    pub fn new_fungible_definition_with_authority(
        definition_target_account: AccountWithMetadata,
        holding_target_account: AccountWithMetadata,
        name: String,
        total_supply: u128,
        mint_authority: Option<nssa_core::account::AccountId>,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(
            token_program::new_definition::new_fungible_definition_with_authority(
                definition_target_account,
                holding_target_account,
                name,
                total_supply,
                mint_authority,
            ),
            vec![],
        ))
    }

    /// Rotate or revoke the mint authority on a fungible token definition.
    /// `new_authority: Some(id)` rotates; `None` permanently revokes (fixed supply).
    #[instruction]
    pub fn set_authority(
        definition_account: AccountWithMetadata,
        new_authority: Option<nssa_core::account::AccountId>,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(
            token_program::set_authority::set_authority(
                definition_account,
                new_authority,
            ),
            vec![],
        ))
    }

    /// Create a new fungible or non-fungible token definition with metadata.
    #[expect(
        clippy::boxed_local,
        reason = "boxed metadata keeps the instruction argument size bounded on the stack"
    )]
    #[instruction]
    pub fn new_definition_with_metadata(
        definition_target_account: AccountWithMetadata,
        holding_target_account: AccountWithMetadata,
        metadata_target_account: AccountWithMetadata,
        new_definition: token_core::NewTokenDefinition,
        metadata: Box<token_core::NewTokenMetadata>,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(
            token_program::new_definition::new_definition_with_metadata(
                definition_target_account,
                holding_target_account,
                metadata_target_account,
                new_definition,
                *metadata,
            ),
            vec![],
        ))
    }

    /// Initialize a token holding account for a given token definition.
    #[instruction]
    pub fn initialize_account(
        ctx: ProgramContext,
        definition_account: AccountWithMetadata,
        account_to_initialize: AccountWithMetadata,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(
            token_program::initialize::initialize_account(
                definition_account,
                account_to_initialize,
                ctx.self_program_id,
            ),
            vec![],
        ))
    }

    /// Burn tokens from the holder's account.
    #[instruction]
    pub fn burn(
        definition_account: AccountWithMetadata,
        user_holding_account: AccountWithMetadata,
        amount_to_burn: u128,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(token_program::burn::burn(
            definition_account,
            user_holding_account,
            amount_to_burn,
        ), vec![]))
    }

    /// Mint new tokens to the holder's account.
    /// Requires an active mint authority on the token definition.
    #[instruction]
    pub fn mint(
        ctx: ProgramContext,
        definition_account: AccountWithMetadata,
        user_holding_account: AccountWithMetadata,
        amount_to_mint: u128,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(token_program::mint::mint(
            definition_account,
            user_holding_account,
            amount_to_mint,
            ctx.self_program_id,
        ), vec![]))
    }

    /// Print a new NFT from the master copy.
    #[instruction]
    pub fn print_nft(
        master_account: AccountWithMetadata,
        printed_account: AccountWithMetadata,
    ) -> SpelResult {
        Ok(spel_framework::SpelOutput::execute(token_program::print_nft::print_nft(
            master_account,
            printed_account,
        ), vec![]))
    }
}
