#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    token::Client as TokenClient,
    Address, Env, Symbol,
};

// ─── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Owner,
    Token,   // USDC contract address
}

// ─── Events ──────────────────────────────────────────────────────────────────

pub fn emit_deposit(env: &Env, from: &Address, amount: i128) {
    let topics = (Symbol::new(env, "deposit"), from.clone());
    env.events().publish(topics, amount);
}

pub fn emit_withdraw(env: &Env, to: &Address, amount: i128) {
    let topics = (Symbol::new(env, "withdraw"), to.clone());
    env.events().publish(topics, amount);
}

pub fn emit_ownership_transferred(env: &Env, old: &Address, new: &Address) {
    let topics = (Symbol::new(env, "ownership_transferred"),);
    env.events().publish(topics, (old.clone(), new.clone()));
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct UsdcVault;

#[contractimpl]
impl UsdcVault {
    // ── Constructor ───────────────────────────────────────────────────────────
    
    /// Initializer: caller passes the owner address explicitly.
    pub fn initialize_with_owner(env: Env, owner: Address, token: Address) {
        if env.storage().instance().has(&DataKey::Owner) {
            panic!("already initialised");
        }

        // Require the declared owner to sign this transaction
        owner.require_auth();

        env.storage().instance().set(&DataKey::Owner, &owner);
        env.storage().instance().set(&DataKey::Token, &token);

        // Extend TTL so storage survives long-lived use (max ledger extension)
        env.storage().instance().extend_ttl(100_000, 100_000);
    }

    // ── Deposit (approve + transfer-from) ────────────────────────────────────
    /// Anyone can deposit USDC into the vault.
    ///
    /// The caller must have already called `approve` on the USDC contract
    /// granting this vault `i128::MAX` (or at least `amount`) allowance.
    /// This function then pulls the funds in via `transfer_from`.
    pub fn deposit(env: Env, from: Address, amount: i128) {
        assert!(amount > 0, "amount must be positive");

        // Caller must authorise the deposit (prevents griefing)
        from.require_auth();

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised");

        let vault = env.current_contract_address();
        let token = TokenClient::new(&env, &token_addr);

        // Pull USDC from `from` into the vault
        token.transfer_from(&vault, &from, &vault, &amount);

        emit_deposit(&env, &from, amount);
    }

    // ── Approve helper (informational) ───────────────────────────────────────
    /// Convenience view: returns the USDC allowance the vault currently holds
    /// over `owner`'s balance (i.e. what the owner approved for this vault).
    pub fn allowance(env: Env, owner: Address) -> i128 {
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised");

        let vault = env.current_contract_address();
        let token = TokenClient::new(&env, &token_addr);
        token.allowance(&owner, &vault)
    }

    // ── Owner-only: Withdraw / Send ───────────────────────────────────────────
    /// Transfer `amount` of USDC from the vault to `to`.
    /// Only the owner may call this.
    pub fn withdraw(env: Env, to: Address, amount: i128) {
        assert!(amount > 0, "amount must be positive");

        // Auth-gate: only the stored owner may proceed
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .expect("not initialised");
        owner.require_auth();

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised");

        let vault = env.current_contract_address();
        let token = TokenClient::new(&env, &token_addr);

        // Safety: ensure the vault actually holds enough
        let bal = token.balance(&vault);
        assert!(bal >= amount, "insufficient vault balance");

        token.transfer(&vault, &to, &amount);

        emit_withdraw(&env, &to, amount);
    }

    // ── Owner-only: Drain entire vault balance ────────────────────────────────
    /// Convenience: withdraw 100% of the vault balance to `to`.
    pub fn drain(env: Env, to: Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .expect("not initialised");
        owner.require_auth();

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised");

        let vault = env.current_contract_address();
        let token = TokenClient::new(&env, &token_addr);

        let bal = token.balance(&vault);
        assert!(bal > 0, "vault is empty");

        token.transfer(&vault, &to, &bal);

        emit_withdraw(&env, &to, bal);
    }

    // ── Owner-only: Transfer ownership ───────────────────────────────────────
    /// Hand ownership to a new address.
    /// Both old and new owner must sign (two-step safety).
    pub fn transfer_ownership(env: Env, new_owner: Address) {
        let old_owner: Address = env
            .storage()
            .instance()
            .get(&DataKey::Owner)
            .expect("not initialised");
        old_owner.require_auth();
        new_owner.require_auth();

        env.storage().instance().set(&DataKey::Owner, &new_owner);
        emit_ownership_transferred(&env, &old_owner, &new_owner);
    }

    // ── Views ─────────────────────────────────────────────────────────────────
    pub fn owner(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Owner)
            .expect("not initialised")
    }

    pub fn token(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised")
    }

    /// Current USDC balance held by the vault.
    pub fn balance(env: Env) -> i128 {
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("not initialised");

        let vault = env.current_contract_address();
        let token = TokenClient::new(&env, &token_addr);
        token.balance(&vault)
    }
}