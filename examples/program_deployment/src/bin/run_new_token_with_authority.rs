use common::transaction::LeeTransaction;
use lee::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use sequencer_service_rpc::RpcClient as _;
use token_core::Instruction;
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().expect("Wallet env not configured");

    let definition_id: AccountId = std::env::args_os()
        .nth(1).unwrap().into_string().unwrap().parse().unwrap();
    let supply_id: AccountId = std::env::args_os()
        .nth(2).unwrap().into_string().unwrap().parse().unwrap();
    let name: String = std::env::args_os()
        .nth(3).unwrap().into_string().unwrap();
    let total_supply: u128 = std::env::args_os()
        .nth(4).unwrap().into_string().unwrap().parse().unwrap();
    let authority_arg = std::env::args_os()
        .nth(5).unwrap().into_string().unwrap();
    let mint_authority: Option<AccountId> = if authority_arg == "none" {
        None
    } else {
        Some(authority_arg.parse().unwrap())
    };

    println!("Creating token '{}' total_supply={} mint_authority={:?}", name, total_supply, mint_authority);

    let program = Program::token();
    let instruction = Instruction::NewFungibleDefinitionWithAuthority {
        name,
        total_supply,
        mint_authority,
    };
    let instruction_data =
        Program::serialize_instruction(instruction).expect("Instruction serialization failed");

    let def_signing_key = wallet_core
        .storage()
        .key_chain()
        .pub_account_signing_key(definition_id)
        .expect("definition account signing key not found");
    let sup_signing_key = wallet_core
        .storage()
        .key_chain()
        .pub_account_signing_key(supply_id)
        .expect("supply account signing key not found");

    let nonces = wallet_core
        .get_accounts_nonces(vec![definition_id, supply_id])
        .await
        .expect("Failed to fetch nonces");

    let signing_keys = [def_signing_key, sup_signing_key];
    let message = Message::try_new(
        program.id(),
        vec![definition_id, supply_id],
        nonces,
        instruction_data,
    )
    .unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let response = wallet_core
        .sequencer_client
        .send_transaction(LeeTransaction::Public(tx))
        .await
        .unwrap();

    println!("✅ Token created. Transaction: {:?}", response);
}
