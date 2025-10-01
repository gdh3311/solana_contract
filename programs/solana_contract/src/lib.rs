use anchor_lang::prelude::*;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const ADMIN_PUBKEY: Pubkey = pubkey!("JAZZAQu3Nz6K2Mdy2y2pJmcWK7VNJW6Bhrwh2Fio1xPj"); 

#[program]
pub mod solana_contract {
    use super::*;

    pub fn deposit(ctx: Context<Deposit>, hash_key: String, amount: u64) -> Result<()> {
        require!(hash_key.len() <= 32, CustomError::KeyTooLong);
        require!(amount > 0, CustomError::InvalidAmount);

        let balance_account = &mut ctx.accounts.balance_account;
        balance_account.hash_key = hash_key.clone();  // String 그대로 저장
        balance_account.balance = amount;

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

        msg!("Deposited {} lamports with key: {}", amount, hash_key);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, hash_key: String) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;

        require!(balance_account.hash_key == hash_key, CustomError::InvalidKey);
        require!(balance_account.balance > 0, CustomError::NoBalance);

        let amount = balance_account.balance;

        **ctx.accounts.balance_account.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount;

        msg!("Withdrawn {} lamports to user, rent sent to admin", amount);
        Ok(())
    }
}

#[account]
pub struct BalanceAccount {
    pub hash_key: String,    // String으로 유지
    pub balance: u64,         
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 4 + 32 + 8,  // 52 bytes
        seeds = [b"balance", hash_key.as_bytes()],
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [b"balance", hash_key.as_bytes()],
        bump,
        close = admin,
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Admin address verified by constraint
    #[account(
        mut,
        constraint = admin.key() == ADMIN_PUBKEY @ CustomError::InvalidAdmin
    )]
    pub admin: UncheckedAccount<'info>, 
}

#[error_code]
pub enum CustomError {
    #[msg("Key is too long. Maximum 32 bytes.")]
    KeyTooLong,
    #[msg("Invalid amount. Must be greater than 0.")]
    InvalidAmount,
    #[msg("Invalid key.")]
    InvalidKey,
    #[msg("No balance found.")]
    NoBalance,
    #[msg("Invalid admin address.")] 
    InvalidAdmin,
}