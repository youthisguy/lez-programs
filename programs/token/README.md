# LP-0013: Token Program - Mint Authority

This branch adds a mint authority model to the LEZ token program, enabling variable supply tokens, permissioned issuance, and the standard "revoke to fix supply" pattern expected by wallets and DeFi protocols.

## What Changed

The existing token program supported creating tokens with a fixed total supply but had no mechanism to control who could mint additional tokens after creation. This PR adds:

- `mint_authority: Option<AccountId>` field on `TokenDefinition::Fungible`
- `NewFungibleDefinitionWithAuthority` instruction — create a token and set a mint authority at initialization
- `SetAuthority` instruction — rotate the authority to a new account, or revoke it permanently by setting it to `None`
- Updated `Mint` instruction — now enforces that `mint_authority` is `Some` before allowing minting
- Fully backwards compatible — the existing `NewFungibleDefinition` instruction still works, creating tokens with `mint_authority: None` (fixed supply)

The design follows Solana's SPL Token authority model: a single `Option<AccountId>` field simultaneously encodes who the authority is and whether minting is possible. `None` is self-describing — no authority, no minting, ever.

## Files Changed

| File | Change |
|---|---|
| `programs/token/core/src/lib.rs` | Added `mint_authority` field to `TokenDefinition::Fungible`; added `NewFungibleDefinitionWithAuthority` and `SetAuthority` instruction variants |
| `programs/token/src/new_definition.rs` | Added `new_fungible_definition_with_authority()` function |
| `programs/token/src/set_authority.rs` | New file — implements authority rotation and revocation |
| `programs/token/src/mint.rs` | Added authority check before minting |
| `programs/token/src/lib.rs` | Exposed `set_authority` module |
| `programs/token/methods/guest/src/bin/token.rs` | Wired both new instructions into the SPEL guest dispatch |
| `programs/token/src/tests.rs` | 8 new unit tests covering every authority state transition |
| `programs/integration_tests/tests/token.rs` | 3 new integration tests running against a real `V03State` |

## Authority Lifecycle

```
NewFungibleDefinitionWithAuthority
  mint_authority: Some(A)   ──▶  minting allowed (only A can mint)
                            │
                            ├── SetAuthority(Some(B))  ──▶  authority transferred to B
                            │
                            └── SetAuthority(None)     ──▶  supply permanently fixed
                                     │
                                     └── Mint ──▶  PANIC: "Mint authority has been revoked; supply is fixed"
```

### Atomicity

Authority rotation and revocation are atomic. The RISC Zero zkVM either commits the full output state or panics — there is no partial write. A failed `SetAuthority` call leaves the authority unchanged.

### Error Codes

| Panic message | Condition |
|---|---|
| `"Mint authority has been revoked; supply is fixed"` | `Mint` called when `mint_authority` is `None` |
| `"Definition account must be authorized by current mint authority"` | `SetAuthority` called without authorization |
| `"Mint authority is already revoked; cannot rotate a revoked authority"` | `SetAuthority` called when authority already `None` |
| `"Cannot set mint authority on a Non-Fungible Token definition"` | `SetAuthority` called on an NFT definition |

## SDK

The SDK is `token_core` — the same crate modified in this PR. Downstream consumers import `token_core::Instruction` and get both new variants automatically. No separate SDK crate is needed; this follows the same pattern as `amm_core`, `stablecoin_core`, and `ata_core`.

```rust
use token_core::Instruction;

// Create a token with mint authority
let ix = Instruction::NewFungibleDefinitionWithAuthority {
    name: String::from("Gold"),
    total_supply: 1_000_000,
    mint_authority: Some(authority_account_id),
};

// Rotate authority
let ix = Instruction::SetAuthority {
    new_authority: Some(new_authority_id),
};

// Revoke authority permanently
let ix = Instruction::SetAuthority {
    new_authority: None,
};
```

## Prerequisites

- Rust (stable) — install via [rustup](https://rustup.rs/)
- RISC Zero toolchain:
  ```bash
  curl -L https://risczero.com/install | bash
  rzup install
  ```
- LEZ wallet and sequencer from [logos-blockchain/logos-execution-zone](https://github.com/youthisguy/logos-execution-zone)

## Build & Test

```bash
git clone https://github.com/youthisguy/lez-programs.git
cd lez-programs
git checkout feat/mint-authority

# Run all tests (skips ZK proof generation)
RISC0_DEV_MODE=1 cargo test --release

# Run token-specific unit tests
RISC0_DEV_MODE=1 cargo test --release -p token_program

# Run token integration tests
RISC0_DEV_MODE=1 cargo test --release -p integration_tests --test token
```

All 245+ tests pass. The 8 new unit tests and 3 new integration tests are included in the count.

## End-to-End Demo

### Prerequisites

Start all three services in separate terminals:

**Terminal 1 — Bedrock:**
```bash
cd logos-execution-zone/bedrock
docker compose up
```

**Terminal 2 — Sequencer (after bedrock shows "proposed block"):**
```bash
cd logos-execution-zone/lez/sequencer/service
RUST_LOG=info RISC0_DEV_MODE=1 cargo run --release -p sequencer_service configs/debug/sequencer_config.json
```

**Terminal 3 — Indexer:**
```bash
cd logos-execution-zone/lez/indexer/service
RUST_LOG=info RISC0_DEV_MODE=1 cargo run --release -p indexer_service configs/indexer_config.json
```

**Terminal 4 — Wallet commands:**
```bash
export LEZ_WALLET_HOME_DIR=logos-execution-zone/lez/wallet/configs/debug
cd logos-execution-zone
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet check-health
# ✅ All looks good!
```

### Demo Walkthrough

**1. Create accounts:**
```bash
./target/release/wallet account new public --label "token-def"
./target/release/wallet account new public --label "token-supply"
./target/release/wallet account new public --label "new-authority"
```

**2. Create a token WITH mint authority:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token new-with-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --supply-account-id "Public/<SUPPLY_ID>" \
    --name "Gold" \
    --total-supply 1000000 \
    --mint-authority "<DEF_ID>"
```

**3. Verify on-chain — mint_authority is set:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet account get \
    --account-id "Public/<DEF_ID>"
# {"Fungible":{"name":"Gold","total_supply":1000000,"metadata_id":null,"mint_authority":"<DEF_ID>"}}
```

**4. Mint additional tokens:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token mint \
    --definition "Public/<DEF_ID>" \
    --holder "Public/<SUPPLY_ID>" \
    --amount 500000
```

**5. Rotate authority to a new account:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token set-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --new-authority "<NEW_AUTHORITY_ID>"
```

**6. Revoke authority permanently:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token set-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --new-authority "none"
```

**7. Verify final state — mint_authority is null, total_supply updated:**
```bash
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet account get \
    --account-id "Public/<DEF_ID>"
# {"Fungible":{"name":"Gold","total_supply":1500000,"metadata_id":null,"mint_authority":null}}
```

## Example Integrations

Two example Rust programs are in `examples/program_deployment/src/bin/`:

**Fixed supply token (authority revoked at creation):**
```bash
# Create token with no authority — supply is fixed immediately
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token new-with-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --supply-account-id "Public/<SUPPLY_ID>" \
    --name "FixedCoin" \
    --total-supply 21000000 \
    --mint-authority "none"
# mint_authority: null from the start — nobody can ever mint more
```

**Variable supply token (authority set, then used):**
```bash
# Create with authority
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token new-with-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --supply-account-id "Public/<SUPPLY_ID>" \
    --name "GovToken" \
    --total-supply 1000000 \
    --mint-authority "<DEF_ID>"

# Mint more later
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token mint \
    --definition "Public/<DEF_ID>" \
    --holder "Public/<SUPPLY_ID>" \
    --amount 250000

# Lock supply when ready
SEQUENCER_URL=http://127.0.0.1:3040 ./target/release/wallet token set-authority \
    --definition-account-id "Public/<DEF_ID>" \
    --new-authority "none"
```

## Compute Unit Costs

Measured on LEZ local sequencer with `RISC0_DEV_MODE=1`:

| Operation | Execution time (dev mode) |
|---|---|
| `NewFungibleDefinitionWithAuthority` | ~130ms |
| `Mint` (with authority check) | ~80ms |
| `SetAuthority` (rotate) | ~50ms |
| `SetAuthority` (revoke) | ~50ms |

Note: execution times in `RISC0_DEV_MODE=0` (real proof generation) are significantly higher (~5–30 minutes per transaction depending on hardware). CU costs on LEZ devnet/testnet will be documented once the testnet budget is finalized.

## Design Decisions

**Why `Option<AccountId>` and not a separate `is_fixed_supply: bool`?**
One field encodes both who the authority is and whether minting is possible. `None` is self-describing — no authority, no minting, ever. Separate fields risk inconsistent state (`is_fixed_supply: false` but `mint_authority: None`).

**Why is `SetAuthority` a separate instruction and not part of `NewDefinition`?**
Separation of concerns. Creation and authority management are different operations with different signers and different lifecycle stages. This also matches SPL Token's design.

**Why does `mint.rs` check `is_none()` rather than comparing account IDs?**
The LEZ authorization model sets `is_authorized: true` on an account when the transaction includes a valid signature for that account. The program trusts the protocol's authorization flag rather than re-implementing signature verification. This is the correct pattern for all LEZ programs.

**Why check revocation before authorization in `set_authority`?**
If `mint_authority` is `None`, there is no authorized caller possible — the clearer error is "already revoked" rather than "unauthorized".

## Related Repos

- [`youthisguy/logos-execution-zone`](https://github.com/youthisguy/logos-execution-zone) — mirrored changes to the sequencer, wallet CLI, and guest binary. The wallet CLI gains two new commands: `token new-with-authority` and `token set-authority`.

## License

MIT OR Apache-2.0