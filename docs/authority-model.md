# Token Mint Authority — Design Specification

## Overview

This document specifies the mint authority model added to the LEZ token program as part of LP-0013. It covers the data model, instruction semantics, authority lifecycle, atomicity guarantees, error codes, and the moderator trust model.

## Data Model

### `TokenDefinition::Fungible`

```rust
TokenDefinition::Fungible {
    name: String,
    total_supply: u128,
    metadata_id: Option<AccountId>,
    mint_authority: Option<AccountId>,  // ← new field
}
```

`mint_authority: Option<AccountId>` encodes two things in one field:

- `Some(account_id)` — that account is the current mint authority. Only a transaction signed by that account's key may mint additional tokens.
- `None` — the supply is permanently fixed. No further minting is possible, ever.

Using a single `Option` instead of a separate `is_fixed_supply: bool` eliminates the risk of inconsistent state (e.g. `is_fixed_supply: false` with `mint_authority: None`). This is the same design used by Solana's SPL Token program.

## Instructions

### `NewFungibleDefinitionWithAuthority`

```rust
Instruction::NewFungibleDefinitionWithAuthority {
    name: String,
    total_supply: u128,
    mint_authority: Option<AccountId>,
}
```

Creates a new fungible token definition with an explicit mint authority. Passing `None` as `mint_authority` creates a permanently fixed supply token at initialization — no separate revocation step is needed.

Required accounts (in order):
1. Token Definition account (uninitialized, authorized)
2. Token Holding account (uninitialized, authorized)

### `SetAuthority`

```rust
Instruction::SetAuthority {
    new_authority: Option<AccountId>,
}
```

Rotates or revokes the mint authority on an existing fungible token definition.

- `Some(new_id)` — transfers authority to `new_id`. The previous authority loses all minting rights immediately.
- `None` — permanently revokes authority. The supply is fixed from this point on. This operation is irreversible.

Required accounts (in order):
1. Token Definition account (initialized, authorized by current mint authority)

### `Mint` (updated)

The existing `Mint` instruction now enforces the authority check before executing:

1. If `mint_authority` is `None` → panic with `"Mint authority has been revoked; supply is fixed"`
2. If `mint_authority` is `Some(_)` but `is_authorized` is `false` → panic with `"Definition authorization is missing"`
3. Otherwise → proceed with minting

## Authority Lifecycle

```
                    ┌─────────────────────────────────────┐
                    │  NewFungibleDefinitionWithAuthority  │
                    │  mint_authority: Some(A)             │
                    └──────────────┬──────────────────────┘
                                   │
                    ┌──────────────▼──────────────────────┐
                    │  ACTIVE: mint_authority = Some(A)    │
                    │  - A can mint more tokens            │
                    │  - A can rotate authority            │
                    │  - A can revoke authority            │
                    └──────┬──────────────┬───────────────┘
                           │              │
          SetAuthority     │              │  SetAuthority
          Some(B)          │              │  None
                           │              │
         ┌─────────────────▼──┐    ┌──────▼──────────────────────┐
         │  ACTIVE: Some(B)   │    │  REVOKED: None               │
         │  (same as above,   │    │  - Minting permanently       │
         │   authority is B)  │    │    disabled                  │
         └────────────────────┘    │  - SetAuthority fails        │
                                   │  - Supply is fixed forever   │
                                   └──────────────────────────────┘
```

### Fixed Supply at Creation

Passing `mint_authority: None` to `NewFungibleDefinitionWithAuthority` skips the active state entirely:

```
NewFungibleDefinitionWithAuthority(mint_authority: None)
  ──▶  REVOKED immediately (supply fixed from block 0)
```

This is equivalent to calling `NewFungibleDefinition` — which also sets `mint_authority: None` implicitly — but makes the intent explicit in the instruction.

## Atomicity

All state transitions are atomic. The RISC Zero zkVM executes the guest program and either:

- Commits the full output state (all account changes are applied), or
- Panics (no state is written — the pre-state is preserved exactly)

There is no mechanism for partial writes. A failed `SetAuthority` call — whether due to authorization failure, revocation check, or NFT check — leaves the definition account's `mint_authority` field unchanged.

This means:
- A rotation that fails leaves the old authority in place.
- A revocation that fails leaves the authority active.
- There is no "undefined" or "in-between" authority state.

## Error Codes

All errors are surfaced as guest panics. The sequencer records them as `ProgramExecutionFailed` with the panic message as the inner string.

| Condition | Panic message |
|---|---|
| `Mint` called when `mint_authority` is `None` | `"Mint authority has been revoked; supply is fixed"` |
| `SetAuthority` called without authorization | `"Definition account must be authorized by current mint authority"` |
| `SetAuthority` called when authority already `None` | `"Mint authority is already revoked; cannot rotate a revoked authority"` |
| `SetAuthority` called on a NonFungible definition | `"Cannot set mint authority on a Non-Fungible Token definition"` |
| `Mint` called without authorization | `"Definition authorization is missing"` |

Error messages are deterministic — the same condition always produces the same message string.

## Authorization Model

The LEZ authorization model works through the `is_authorized` flag on `AccountWithMetadata`. The protocol sets this flag to `true` when the transaction includes a valid signature for that account's keypair.

Programs trust this flag rather than re-implementing signature verification. This is the correct and consistent pattern across all LEZ programs (token, AMM, stablecoin, ATA).

For `SetAuthority`:
- The definition account must have `is_authorized: true`
- This means the transaction must be signed by the key corresponding to the current `mint_authority` account ID
- If `mint_authority` is `Some(A)`, the transaction must include a signature from A's keypair

For `Mint`:
- The definition account must have `is_authorized: true`
- This means the transaction must be signed by the key corresponding to `mint_authority`
- The check is: `mint_authority.is_some()` (supply not fixed) AND `is_authorized` (correct key signed)

## Backwards Compatibility

The existing `NewFungibleDefinition` instruction is unchanged. It implicitly sets `mint_authority: None`, creating a fixed supply token. All programs that use `NewFungibleDefinition` continue to work without modification.

The `mint_authority` field is appended to `TokenDefinition::Fungible`'s Borsh serialization. Existing on-chain accounts created before this change will fail to deserialize with the new schema — this is expected for a breaking schema change and is handled by requiring a fresh deployment.

## Compute Unit Costs

Measured on LEZ devnet (local sequencer standalone mode — devnet/localnet).
Run with `RISC0_DEV_MODE=0` — real ZK proofs generated.
Reproducible: clone repo, run `scripts/demo.sh` with `RISC0_DEV_MODE=0`, observe `execution time:` lines in sequencer logs.

| Operation | Tx Hash | Block | Execution Time (RISC0_DEV_MODE=0) |
|---|---|---|---|
| `NewFungibleDefinitionWithAuthority` | `14197f9113ff000e81b7545c671942b286ef19bae7122ba280a0a620b8e01ca1` | 410 | 15.92ms |
| `Mint` (authority active) | `99f00dbe40600d0c8bb745b74980c2241f1e7a6daa1291f5cef6b9ea27c82bd9` | 411 | 19.29ms |
| `SetAuthority` (rotate) | `d865e26dfb5f82a5528aa9a0882307a73b00ffc4fa7825f0e7b5d0888d5c87fc` | 414 | 13.40ms |
| `SetAuthority` (revoke to None) | `9408ef7ffd3efdbafbe2dd5bf243da32edd1a4d52f9709b5cfc92cb696b8956e` | 415 | 15.74ms |
| `Mint` (rejected — authority revoked) | `5228cc62094a91e479b86a3aee067809f18674465ac72d8623d1ed770ab496de` | 416 | 9.84ms |

Rejected operations cost ~38% less than successful ones because execution halts at the
authority guard before any account writes — confirming rejection is via the correct code
path (`"Mint authority has been revoked; supply is fixed"`), not a side effect.

## Moderator Trust Model

The mint authority is fully trusted within the scope of a single token definition. The protocol enforces:

- Only the current authority can rotate or revoke itself
- Revocation is permanent and cannot be undone by anyone
- No one can "re-grant" authority after revocation — not even the original creator

There is no multi-sig or timelock on authority operations in this implementation. A single keypair controls the authority. Applications requiring shared governance over minting should implement a multisig wrapper program (see LP-0002) that holds the mint authority keypair.

## Threat Model

**What the protocol prevents:**
- Unauthorized minting (any account other than the current authority)
- Minting after revocation
- Re-granting authority after revocation
- Partial authority state (impossible due to RISC Zero atomicity)

**What the protocol does NOT prevent:**
- Authority key compromise — if the authority's private key is stolen, the attacker can mint or rotate authority before the legitimate holder can revoke
- Front-running on `SetAuthority` — if an attacker observes a revocation transaction in the mempool, they could submit a mint transaction first (mempool ordering is not guaranteed)
- The original creator reclaiming authority — once authority is rotated to another account, the original creator has no special power to reclaim it

**Mitigations for key compromise:**
- Rotate authority to a new keypair immediately if compromise is suspected
- Use a multisig program as the authority for high-value tokens

## Known Limitations

- No freeze authority (out of scope per LP-0013)
- No capped supply with a cap distinct from `total_supply` (out of scope)
- Authority check uses `is_authorized` flag which requires the transaction to be signed by the authority keypair — there is no support for program-owned authorities (PDAs) as mint authorities in this implementation