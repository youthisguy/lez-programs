# LP-0013: Token Program — Mint Authority

This fork of [logos-blockchain/lez-programs](https://github.com/logos-blockchain/lez-programs) adds a mint authority model to the LEZ token program, enabling variable supply tokens, permissioned issuance, and the standard "revoke to fix supply" pattern expected by wallets and DeFi protocols.

For the LP-0013 contribution, what changed, and how to run it — see below.
For the wallet CLI additions (two new commands: `token new-with-authority` and `token set-authority`) — see the supporting fork: [youthisguy/logos-execution-zone](https://github.com/youthisguy/logos-execution-zone).
Everything else is the upstream lez-programs codebase.

---

## What was added

- `mint_authority: Option<AccountId>` field on `TokenDefinition::Fungible`
- `NewFungibleDefinitionWithAuthority` instruction — create a token with a mint authority at initialization
- `SetAuthority` instruction — rotate authority to a new account, or revoke it permanently by passing `None`
- Updated `Mint` instruction — enforces the authority check before any state write
- Fully backwards compatible — the existing `NewFungibleDefinition` instruction is unchanged

The design follows Solana's SPL Token: a single `Option<AccountId>` encodes both who the authority is and whether minting is possible. `None` is self-describing — no authority, no minting, ever.

## Admin Authority Library (RFP-001)

The mint authority logic is implemented as a standalone, reusable crate — [`crates/lez-authority`](crates/lez-authority) - This satisfies [RFP-001: Admin Authority Library](https://github.com/logos-co/rfp/blob/master/RFPs/RFP-001-admin-authority-lib.md), which calls for standardised access control that any LEZ program can adopt.

`lez-authority` provides:
- `Authority` — wraps `Option<AccountId>`; `Some(id)` is an active authority, `None` is permanently renounced
- `Authority::rotate()` — transfer authority to a new signer (requires current authorization)
- `Authority::revoke()` — permanently renounce authority (irreversible)
- `Authority::require()` — gate a privileged instruction; returns `AuthorityError::Unauthorized` or `AuthorityError::Renounced`
- `require_authority!` macro — panics with a clear message in LEZ guest programs

The token program is the first consumer: `mint.rs` and `set_authority.rs` both call into `lez-authority` instead of implementing authorization checks inline. See [`crates/lez-authority/README.md`](crates/lez-authority/README.md) for integration instructions and a usage example.

---

## Repositories

| Repo | Purpose |
|---|---|
| [youthisguy/lez-programs](https://github.com/youthisguy/lez-programs) ← **this repo** | Token program changes, `token_core` SDK, integration tests, demo script |
| [youthisguy/logos-execution-zone](https://github.com/youthisguy/logos-execution-zone) | Sequencer, wallet CLI (`token new-with-authority`, `token set-authority`) |

---

## Documentation

| Document | Description |
|---|---|
| [programs/token/README.md](programs/token/README.md) | End-to-end usage: deploy steps, program addresses, CLI instructions for minting, rotating, and revoking authority |
| [docs/authority-model.md](docs/authority-model.md) | Full design spec: data model, instruction semantics, authority lifecycle diagram, atomicity proof, error codes, authorization model, backwards compatibility, threat model |
| [artifacts/token-idl.json](artifacts/token-idl.json) | SPEL-generated IDL for the updated token program (regenerate with `cargo run -p idl-gen`) |

---

## Example Integrations

Two example Rust programs are in [`examples/program_deployment/src/bin/`](examples/program_deployment/src/bin/):

| Example | Description |
|---|---|
| [`run_new_token_with_authority.rs`](examples/program_deployment/src/bin/run_new_token_with_authority.rs) | Variable supply token — creates a token with an active mint authority, mints additional supply, then rotates the authority |
| [`run_new_fixed_supply_token.rs`](examples/program_deployment/src/bin/run_new_fixed_supply_token.rs) | Fixed supply token — creates a token with `mint_authority: None` at initialization; no revocation step needed |

---

## Build & Test

```bash
git clone https://github.com/youthisguy/lez-programs.git
cd lez-programs

# All tests (skips ZK proof generation)
RISC0_DEV_MODE=1 cargo test --release

# Token unit tests only
RISC0_DEV_MODE=1 cargo test --release -p token_program

# Token integration tests only
RISC0_DEV_MODE=1 cargo test --release -p integration_tests --test token
```

CI status: [![CI](https://github.com/youthisguy/lez-programs/actions/workflows/ci.yml/badge.svg)](https://github.com/youthisguy/lez-programs/actions)

---

## End-to-End Demo

The demo script runs the full authority lifecycle against a real local sequencer with `RISC0_DEV_MODE=0`

### Prerequisites

Clone and build the supporting repo first:

```bash
git clone https://github.com/youthisguy/logos-execution-zone.git
cd logos-execution-zone
cargo build --release
```

Start all three services in separate terminals:

```bash
# Terminal 1 — Bedrock
cd logos-execution-zone/bedrock && docker compose up

# Terminal 2 — Sequencer (after bedrock shows "proposed block")
cd logos-execution-zone/lez/sequencer/service
RUST_LOG=info RISC0_DEV_MODE=0 cargo run --release -p sequencer_service configs/debug/sequencer_config.json

# Terminal 3 — Indexer
cd logos-execution-zone/lez/indexer/service
RUST_LOG=info cargo run --release -p indexer_service configs/indexer_config.json
```

### Run

```bash
RISC0_DEV_MODE=0 \
WALLET_BIN=/path/to/logos-execution-zone/target/release/wallet \
LEZ_WALLET_HOME_DIR=/path/to/logos-execution-zone/lez/wallet/configs/debug \
bash scripts/demo.sh
```

See [`scripts/demo.sh`](scripts/demo.sh) for what each step does. Execution times appear in the sequencer logs as `execution time:` lines.

---

## Compute Unit Costs

Measured on local LEZ sequencer (standalone mode) with `RISC0_DEV_MODE=0`. Reproducible via `scripts/demo.sh` as above.

| Operation | Tx Hash | Block | Execution Time |
|---|---|---|---|
| `NewFungibleDefinitionWithAuthority` | `14197f9113ff000e81b7545c671942b286ef19bae7122ba280a0a620b8e01ca1` | 410 | 15.92ms |
| `Mint` (authority active) | `99f00dbe40600d0c8bb745b74980c2241f1e7a6daa1291f5cef6b9ea27c82bd9` | 411 | 19.29ms |
| `SetAuthority` (rotate) | `d865e26dfb5f82a5528aa9a0882307a73b00ffc4fa7825f0e7b5d0888d5c87fc` | 414 | 13.40ms |
| `SetAuthority` (revoke to None) | `9408ef7ffd3efdbafbe2dd5bf243da32edd1a4d52f9709b5cfc92cb696b8956e` | 415 | 15.74ms |
| `Mint` (rejected — authority revoked) | `5228cc62094a91e479b86a3aee067809f18674465ac72d8623d1ed770ab496de` | 416 | 9.84ms |

Rejected operations cost ~38% less than successful ones — execution halts at the authority guard before any account writes, confirming rejection is via the correct code path.

> **Note:**  These measurements use local sequencer executor timing with real proof generation (`RISC0_DEV_MODE=0`). Testnet CU measurements will be added once the testnet exposes this data.

---

## Video Demo

Narrated walkthrough showing terminal output with `RISC0_DEV_MODE=0` active during proof generation:
[https://youtu.be/mbNpOoOs7T4](https://youtu.be/mbNpOoOs7T4)

---

## License
 
[MIT](LICENSE)