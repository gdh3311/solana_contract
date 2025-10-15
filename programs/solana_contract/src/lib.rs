use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash as sha256_hash;
use anchor_lang::solana_program::keccak::hash as keccak_hash;
use bs58;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

#[program]
pub mod solana_contract {
    use super::*;

    /// 💰 Deposit: h2(=hashed key)에 대한 예치
    pub fn deposit(ctx: Context<Deposit>, hash_key: String, amount: u64) -> Result<()> {
        require!(hash_key.len() <= 64, CustomError::KeyTooLong);
        require!(amount > 0, CustomError::InvalidAmount);

        let balance_account = &mut ctx.accounts.balance_account;
        balance_account.hash_key = hash_key.clone();

        // SOL transfer to PDA
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.balance_account.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.balance_account.to_account_info(),
            ],
        )?;

        msg!("✅ Deposited {} lamports for hash_key(h2): {}", amount, hash_key);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, h1: String) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;

        let computed_h2 = h2_pattern(h1.clone());

        require!(
            computed_h2 == balance_account.hash_key,
            CustomError::InvalidKey
        );

        let total_lamports = balance_account.to_account_info().lamports();

        **balance_account.to_account_info().try_borrow_mut_lamports()? -= total_lamports;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += total_lamports;

        ctx.accounts.balance_account.close(ctx.accounts.user.to_account_info())?;

        msg!("💸 Withdrawn {} lamports to user {}", total_lamports, ctx.accounts.user.key());
        Ok(())
    }
}

/// h2 = keccak → sha256 → keccak → sha256 → keccak
pub fn h2_pattern(input: String) -> String {
    const H2_DOMAIN: &[u8] = b"kuching";
    let mut current = [H2_DOMAIN, input.as_bytes()].concat();

    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();

    bs58::encode(current).into_string()
}

#[account]
pub struct BalanceAccount {
    pub hash_key: String, // h2 저장 (h1 검증용)
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = user,
        space =56 , 
        seeds = [b"balance", hash_key.as_bytes()],
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h1: String)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        close = user,
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum CustomError {
    #[msg("Key is too long. Maximum 64 bytes.")]
    KeyTooLong,
    #[msg("Invalid amount. Must be greater than 0.")]
    InvalidAmount,
    #[msg("Invalid key — h1 does not match stored hash.")]
    InvalidKey,
    #[msg("No balance found.")]
    NoBalance,
}
