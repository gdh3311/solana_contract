use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash as sha256_hash;
use anchor_lang::solana_program::keccak::hash as keccak_hash;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

#[program]
pub mod solana_contract {
    use super::*;

    pub fn deposit(ctx: Context<Deposit>, hash_key: [u8; 32], amount: u64) -> Result<()> {
        require!(amount > 0, CustomError::InvalidAmount);
        ctx.account.
        let balance_account = &mut ctx.accounts.balance_account;
        balance_account.hash_key = hash_key;
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

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, h1: [u8; 32]) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;

        let computed_h2 = h2_pattern(h1);

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

pub fn h2_pattern(input: [u8; 32]) -> [u8; 32] {
    const H2_DOMAIN: &[u8] = b"kuching";
    let mut current = [H2_DOMAIN, &input[..]].concat();

    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();

    current.try_into().expect("Hash output should be 32 bytes")
}


#[account]
pub struct BalanceAccount {
    pub hash_key: [u8; 32],
}

#[derive(Accounts)]
#[instruction(hash_key: [u8; 32])]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 32, 
        seeds = [b"balance", hash_key.as_ref()],
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h1: [u8; 32])]
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
    #[msg("Invalid amount. Must be greater than 0.")]
    InvalidAmount,
    #[msg("Invalid key — h1 does not match stored hash.")]
    InvalidKey,
    #[msg("No balance found.")]
    NoBalance,
}