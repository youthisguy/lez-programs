//! # lez-authority
//!
//! A reusable single-admin authority library for LEZ programs, satisfying RFP-001.
//!
//! Provides standardised access control for LEZ programs where privileged
//! functions can only be called by a designated admin authority. The authority
//! can transfer control to a new signer or permanently renounce it.
//!
//! ## Usage
//!
//! Add to your program's `Cargo.toml`:
//! ```toml
//! [dependencies]
//! lez-authority = { path = "../../crates/lez-authority" }
//! ```
//!
//! Gate a privileged instruction:
//! ```rust,ignore
//! use lez_authority::{Authority, AuthorityError};
//!
//! pub fn my_privileged_instruction(
//!     is_authorized: bool,
//!     current_authority: Option<AccountId>,
//! ) -> Result<(), AuthorityError> {
//!     let auth = Authority::from_option(current_authority);
//!     auth.require(is_authorized)?;
//!     // ... privileged logic
//!     Ok(())
//! }
//! ```

use nssa_core::account::AccountId;

/// Single-admin authority state.
///
/// Wraps `Option<AccountId>`:
/// - `Some(id)` — authority is active; only `id` may call privileged instructions.
/// - `None` — authority has been permanently renounced; no further privileged calls
///   are possible. This state is terminal and cannot be reversed.
///
/// There can only be one admin authority at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority(Option<AccountId>);

impl Authority {
    /// Create an active authority with the given account ID.
    pub fn new(id: AccountId) -> Self {
        Self(Some(id))
    }

    /// Create a permanently renounced authority.
    /// This is equivalent to calling `revoke()` — the state is terminal.
    pub fn renounced() -> Self {
        Self(None)
    }

    /// Construct from an `Option<AccountId>` as stored on-chain.
    pub fn from_option(opt: Option<AccountId>) -> Self {
        Self(opt)
    }

    /// Convert back to `Option<AccountId>` for on-chain storage.
    pub fn into_option(self) -> Option<AccountId> {
        self.0
    }

    /// Returns `true` if the authority has been permanently renounced.
    pub fn is_renounced(&self) -> bool {
        self.0.is_none()
    }

    /// Returns `true` if the authority is still active.
    pub fn is_active(&self) -> bool {
        self.0.is_some()
    }

    /// Returns the current authority account ID, if active.
    pub fn account_id(&self) -> Option<AccountId> {
        self.0
    }

    /// Check that the caller is authorized to perform a privileged action.
    ///
    /// The `is_authorized` flag is set by the LEZ protocol when the transaction
    /// includes a valid signature from the authority account's keypair.
    ///
    /// # Errors
    /// - [`AuthorityError::Renounced`] if the authority has been permanently revoked.
    /// - [`AuthorityError::Unauthorized`] if `is_authorized` is `false`.
    pub fn require(&self, is_authorized: bool) -> Result<(), AuthorityError> {
        if self.is_renounced() {
            return Err(AuthorityError::Renounced);
        }
        if !is_authorized {
            return Err(AuthorityError::Unauthorized);
        }
        Ok(())
    }

    /// Transfer authority to a new account ID.
    ///
    /// # Errors
    /// - [`AuthorityError::Renounced`] if the authority has already been renounced.
    /// - [`AuthorityError::Unauthorized`] if `is_authorized` is `false`.
    pub fn rotate(
        &mut self,
        new_authority: AccountId,
        is_authorized: bool,
    ) -> Result<(), AuthorityError> {
        self.require(is_authorized)?;
        self.0 = Some(new_authority);
        Ok(())
    }

    /// Permanently renounce the authority.
    ///
    /// After calling this, [`is_renounced`](Self::is_renounced) returns `true`
    /// and no further privileged calls are possible. This operation is irreversible.
    ///
    /// # Errors
    /// - [`AuthorityError::Renounced`] if the authority has already been renounced.
    /// - [`AuthorityError::Unauthorized`] if `is_authorized` is `false`.
    pub fn revoke(&mut self, is_authorized: bool) -> Result<(), AuthorityError> {
        self.require(is_authorized)?;
        self.0 = None;
        Ok(())
    }
}

/// Errors produced by authority checks.
///
/// In LEZ guest programs, these are surfaced as panics since the prover
/// catches panics and rejects the transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    /// The caller is not the current authority.
    Unauthorized,
    /// The authority has been permanently renounced.
    /// No privileged actions are possible in this state.
    Renounced,
}

impl core::fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AuthorityError::Unauthorized => {
                write!(f, "Unauthorized: caller is not the current authority")
            }
            AuthorityError::Renounced => {
                write!(f, "Renounced: authority has been permanently revoked")
            }
        }
    }
}

/// Convenience macro to assert authority in LEZ guest programs.
///
/// Panics with a clear message if the authority check fails,
/// following the LEZ guest program convention.
///
/// # Example
/// ```rust,ignore
/// require_authority!(authority, is_authorized);
/// ```
#[macro_export]
macro_rules! require_authority {
    ($authority:expr, $is_authorized:expr) => {
        match $authority.require($is_authorized) {
            Ok(()) => {}
            Err(lez_authority::AuthorityError::Renounced) => {
                panic!("AuthorityError::Renounced: authority has been permanently revoked")
            }
            Err(lez_authority::AuthorityError::Unauthorized) => {
                panic!("AuthorityError::Unauthorized: caller is not the current authority")
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_account_id(byte: u8) -> AccountId {
        AccountId::new([byte; 32])
    }

    #[test]
    fn test_new_authority_is_active() {
        let auth = Authority::new(test_account_id(1));
        assert!(auth.is_active());
        assert!(!auth.is_renounced());
        assert_eq!(auth.account_id(), Some(test_account_id(1)));
    }

    #[test]
    fn test_renounced_authority_is_inactive() {
        let auth = Authority::renounced();
        assert!(!auth.is_active());
        assert!(auth.is_renounced());
        assert_eq!(auth.account_id(), None);
    }

    #[test]
    fn test_require_succeeds_when_authorized() {
        let auth = Authority::new(test_account_id(1));
        assert!(auth.require(true).is_ok());
    }

    #[test]
    fn test_require_fails_when_unauthorized() {
        let auth = Authority::new(test_account_id(1));
        assert_eq!(auth.require(false), Err(AuthorityError::Unauthorized));
    }

    #[test]
    fn test_require_fails_when_renounced() {
        let auth = Authority::renounced();
        assert_eq!(auth.require(true), Err(AuthorityError::Renounced));
    }

    #[test]
    fn test_rotate_transfers_authority() {
        let mut auth = Authority::new(test_account_id(1));
        assert!(auth.rotate(test_account_id(2), true).is_ok());
        assert_eq!(auth.account_id(), Some(test_account_id(2)));
    }

    #[test]
    fn test_rotate_fails_when_unauthorized() {
        let mut auth = Authority::new(test_account_id(1));
        assert_eq!(
            auth.rotate(test_account_id(2), false),
            Err(AuthorityError::Unauthorized)
        );
        // Authority unchanged
        assert_eq!(auth.account_id(), Some(test_account_id(1)));
    }

    #[test]
    fn test_rotate_fails_when_renounced() {
        let mut auth = Authority::renounced();
        assert_eq!(
            auth.rotate(test_account_id(1), true),
            Err(AuthorityError::Renounced)
        );
    }

    #[test]
    fn test_revoke_renounces_authority() {
        let mut auth = Authority::new(test_account_id(1));
        assert!(auth.revoke(true).is_ok());
        assert!(auth.is_renounced());
    }

    #[test]
    fn test_revoke_fails_when_unauthorized() {
        let mut auth = Authority::new(test_account_id(1));
        assert_eq!(auth.revoke(false), Err(AuthorityError::Unauthorized));
        // Authority still active
        assert!(auth.is_active());
    }

    #[test]
    fn test_revoke_fails_when_already_renounced() {
        let mut auth = Authority::renounced();
        assert_eq!(auth.revoke(true), Err(AuthorityError::Renounced));
    }

    #[test]
    fn test_from_option_some() {
        let auth = Authority::from_option(Some(test_account_id(5)));
        assert!(auth.is_active());
    }

    #[test]
    fn test_from_option_none() {
        let auth = Authority::from_option(None);
        assert!(auth.is_renounced());
    }

    #[test]
    fn test_into_option_active() {
        let auth = Authority::new(test_account_id(3));
        assert_eq!(auth.into_option(), Some(test_account_id(3)));
    }

    #[test]
    fn test_into_option_renounced() {
        let auth = Authority::renounced();
        assert_eq!(auth.into_option(), None);
    }

    #[test]
    fn test_full_lifecycle() {
        // Init with authority A
        let mut auth = Authority::new(test_account_id(1));
        assert!(auth.require(true).is_ok());

        // Rotate to B
        auth.rotate(test_account_id(2), true).unwrap();
        assert_eq!(auth.account_id(), Some(test_account_id(2)));

        // Old authority no longer valid (simulated by is_authorized=false)
        assert_eq!(auth.require(false), Err(AuthorityError::Unauthorized));

        // New authority revokes
        auth.revoke(true).unwrap();
        assert!(auth.is_renounced());

        // No further actions possible
        assert_eq!(auth.require(true), Err(AuthorityError::Renounced));
    }
}
