# lez-authority

A reusable single-admin authority library for LEZ programs, satisfying [RFP-001: Admin Authority Library](https://github.com/logos-co/rfp/blob/master/RFPs/RFP-001-admin-authority-lib.md).

Provides standardised access control for LEZ programs where privileged functions can only be called by a designated admin authority. The authority can transfer control to a new signer or permanently renounce it. There can only be one admin authority at a time.

## Why

Without a shared library, every LEZ program that needs "only the admin can call this" logic ends up re-implementing it slightly differently — inconsistent error messages, inconsistent revocation semantics, inconsistent edge-case handling. `lez-authority` gives every program the same primitive, the same error types, and the same tested behavior.

## Install

Add to your program's `Cargo.toml`:

```toml
[dependencies]
lez-authority = { path = "../../crates/lez-authority" }
```

(Adjust the relative path to wherever your program lives relative to `crates/lez-authority`.)

## Core type

```rust
pub struct Authority(Option<AccountId>);
```

`Authority` wraps `Option<AccountId>` — the same representation you'd store on-chain in an account's data:

- `Some(id)` — authority is active; only `id` may call privileged instructions.
- `None` — authority has been permanently renounced. This state is terminal.

There's no separate `is_revoked: bool` to keep in sync — the `Option` *is* the state.

## API

| Method | Description |
|---|---|
| `Authority::new(id)` | Construct an active authority owned by `id` |
| `Authority::renounced()` | Construct an already-revoked authority |
| `Authority::from_option(opt)` | Build from on-chain `Option<AccountId>` storage |
| `.into_option()` | Convert back to `Option<AccountId>` for on-chain storage |
| `.is_active()` / `.is_renounced()` | Query current state |
| `.account_id()` | Get the current authority's `AccountId`, if active |
| `.require(is_authorized)` | Gate a privileged call. Errors if renounced or unauthorized |
| `.rotate(new_id, is_authorized)` | Transfer authority to `new_id`. Errors if renounced or unauthorized |
| `.revoke(is_authorized)` | Permanently renounce. Errors if already renounced or unauthorized |

## Errors

```rust
pub enum AuthorityError {
    Unauthorized,  // caller is not the current authority
    Renounced,     // authority has been permanently revoked
}
```

Both variants implement `Display`, so `panic!("{e}")` in a guest program produces a clear, deterministic message that the sequencer surfaces as the transaction's rejection reason.

## Usage example

This is the actual pattern used by the LEZ token program to gate minting:

```rust
use lez_authority::Authority;

pub fn mint(
    definition_account: AccountWithMetadata,
    /* ... */
) -> Vec<AccountPostState> {
    let mut definition = TokenDefinition::try_from(&definition_account.account.data)
        .expect("Definition account must be valid");

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

    // ... proceed with minting
}
```

And gating authority rotation/revocation:

```rust
use lez_authority::Authority;

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
                Some(new_id) => auth
                    .rotate(new_id, definition_account.is_authorized)
                    .unwrap_or_else(|e| panic!("{e}")),
                None => auth
                    .revoke(definition_account.is_authorized)
                    .unwrap_or_else(|e| panic!("{e}")),
            }

            *mint_authority = auth.into_option();
        }
        TokenDefinition::NonFungible { .. } => {
            panic!("Cannot set mint authority on a Non-Fungible Token definition");
        }
    }

    // ... write post-state
}
```

## The `require_authority!` macro

For guest programs that prefer macro-style gating over manual `match`/`unwrap_or_else`:

```rust
use lez_authority::{Authority, require_authority};

let auth = Authority::from_option(stored_authority);
require_authority!(auth, is_authorized);
// continues only if authorized; panics with a clear message otherwise
```

## Reference integration

The LEZ token program (`programs/token/src/mint.rs` and `programs/token/src/set_authority.rs`) is the reference consumer of this library. Read those two files for a complete, tested, production integration — including how the on-chain `Option<AccountId>` field round-trips through `Authority::from_option` / `.into_option()`.

## Design notes

**Why a single `Authority` type instead of free functions?**
Bundling the `Option<AccountId>` state with its operations means `rotate`/`revoke` can enforce their own preconditions (`require()` runs first) without the caller having to remember to check first. Misuse is harder.

**Why does `require()` check "renounced" before "unauthorized"?**
If authority has been renounced, there is no valid signer who could possibly satisfy the check — `Renounced` is the more informative error regardless of the `is_authorized` flag's value. This ordering is centralized here so every consumer gets it for free, rather than each program deciding independently.

**Why is revocation irreversible?**
This mirrors Solana's SPL Token `set_authority(None)` semantics. A revoked authority cannot be "re-granted" by anyone, including the original holder — this is what makes "fixed supply" or "config locked forever" a credible, verifiable claim rather than a soft convention.

**Why no multisig support?**
Out of scope for this library — `Authority` models exactly one signer. Programs needing shared governance over a privileged action should have a multisig program *be* the authority (i.e., the `AccountId` held by `Authority` is itself a multisig program's PDA), rather than `lez-authority` reimplementing multisig internally.

## Overhead

`Authority` is a zero-cost wrapper around `Option<AccountId>` — identical in memory layout to the field it wraps. There are no additional accounts, no additional instruction fields, and no serialization overhead introduced by routing a check through this library instead of writing the equivalent `match` inline.

## Tests

```bash
cargo test -p lez-authority
```

16 unit tests cover every method in isolation: construction, state queries, `require`/`rotate`/`revoke` success and failure paths, and a full lifecycle test (init → rotate → revoke → confirm no further action possible).
