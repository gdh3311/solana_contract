use anchor_lang::prelude::*;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const ADMIN_PUBKEY: Pubkey = pubkey!("JAZZAQu3Nz6K2Mdy2y2pJmcWK7VNJW6Bhrwh2Fio1xPj");
const USER_RENT_PERCENTAGE: u64 = 30; // 유저는 30%만 부담
const FIXED_RENT: u64 = 1_253_000; 

#[program]
pub mod solana_contract {
    use super::*;

    pub fn deposit(ctx: Context<Deposit>, hash_key: String, amount: u64) -> Result<()> {
        require!(hash_key.len() <= 32, CustomError::KeyTooLong);
        require!(amount > 0, CustomError::InvalidAmount);

        let balance_account = &mut ctx.accounts.balance_account;
        balance_account.hash_key = hash_key.clone();
        balance_account.balance = amount;

        // Rent 계산
        let lent =Rent::get()?.minimum_balance(52);
        let user_rent_portion = lent
            .checked_mul(USER_RENT_PERCENTAGE)
            .ok_or(CustomError::Overflow)?
            .checked_div(100)
            .ok_or(CustomError::Overflow)?;

        // 1. User가 예치금(amount)을 balance_account에 전송
        let ix_amount = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.balance_account.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix_amount,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.balance_account.to_account_info(),
            ],
        )?;

        // 2. User가 rent 30%를 admin에게 직접 전송
        let ix_rent = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.admin.key(),
            user_rent_portion,
        );
        anchor_lang::solana_program::program::invoke(
            &ix_rent,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.admin.to_account_info(),
            ],
        )?;

        msg!("Deposited {} lamports by {}", amount, ctx.accounts.user.key());
        msg!("User paid {}% of rent ({} lamports) to admin", USER_RENT_PERCENTAGE, user_rent_portion);
        msg!("Admin will pay {}% of rent: {} lamports", 100 - USER_RENT_PERCENTAGE, FIXED_RENT - user_rent_portion);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, hash_key: String) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;

        require!(balance_account.hash_key == hash_key, CustomError::InvalidKey);
        require!(balance_account.balance > 0, CustomError::NoBalance);

        let amount = balance_account.balance;

        // User(출금자)에게 예치금만 반환
        **ctx.accounts.balance_account.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount;

        // close = admin으로 설정되어 있어서 rent 전액이 admin에게 반환됨
        msg!("Withdrawn {} lamports to user {}", amount, ctx.accounts.user.key());
        msg!("Full rent ({} lamports) will be returned to admin", FIXED_RENT);
        Ok(())
    }
}

#[account]
pub struct BalanceAccount {
    pub hash_key: String,
    pub balance: u64,
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = admin,  // admin이 70% rent 부담
        space = 8 + 4 + 32 + 8,  // ⭐ space 계산 수정 (32 제거)
        seeds = [b"balance", hash_key.as_bytes()],
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Admin pays 70% of rent and receives 30% from user
    #[account(
        mut,
        constraint = admin.key() == ADMIN_PUBKEY @ CustomError::InvalidAdmin
    )]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [b"balance", hash_key.as_bytes()],
        bump,
        close = admin,  // ⭐ Rent 전액을 admin에게 반환!
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>, // 출금하는 사람 (B)

    /// CHECK: Admin receives all rent back
    #[account(
        mut,
        constraint = admin.key() == ADMIN_PUBKEY @ CustomError::InvalidAdmin
    )]
    pub admin: AccountInfo<'info>,  // ⭐ admin이 rent 전액 받음

    pub system_program: Program<'info, System>,
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
    #[msg("Arithmetic overflow.")]
    Overflow,
    // InvalidDepositor 에러도 제거 ✅
}