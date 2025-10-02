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
        balance_account.depositor = ctx.accounts.user.key(); // ⭐ 입금자 저장

        // Rent 계산
        let user_rent_portion = FIXED_RENT
            .checked_mul(USER_RENT_PERCENTAGE)
            .ok_or(CustomError::Overflow)?
            .checked_div(100)
            .ok_or(CustomError::Overflow)?;

        // User가 입금액 + rent 30% 전송
        let total_from_user = amount
            .checked_add(user_rent_portion)
            .ok_or(CustomError::Overflow)?;

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.balance_account.key(),
            total_from_user,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.balance_account.to_account_info(),
            ],
        )?;

        msg!("Deposited {} lamports by {}", amount, ctx.accounts.user.key());
        msg!("User paid {}% of rent: {} lamports", USER_RENT_PERCENTAGE, user_rent_portion);
        msg!("Admin paid {}% of rent: {} lamports", 100 - USER_RENT_PERCENTAGE, FIXED_RENT - user_rent_portion);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, hash_key: String) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;

        require!(balance_account.hash_key == hash_key, CustomError::InvalidKey);
        require!(balance_account.balance > 0, CustomError::NoBalance);

        let amount = balance_account.balance;

        // User에게 예치금만 반환
        **ctx.accounts.balance_account.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount;

        // close = depositor로 rent를 원래 입금자에게 반환! ⭐
        msg!("Withdrawn {} lamports to user", amount);
        msg!("Rent {} lamports returned to original depositor: {}", FIXED_RENT, balance_account.depositor);
        Ok(())
    }
}

#[account]
pub struct BalanceAccount {
    pub hash_key: String,
    pub balance: u64,
    pub depositor: Pubkey, // ⭐ 원래 입금자 주소 저장
}

#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 4 + 32 + 8 + 32, // ⭐ +32 for Pubkey
        seeds = [b"balance", hash_key.as_bytes()],
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Admin pays 70% of rent
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
        close = depositor, // ⭐ Rent를 원래 입금자에게 반환!
    )]
    pub balance_account: Account<'info, BalanceAccount>,

    #[account(mut)]
    pub user: Signer<'info>, // 출금하는 사람

    /// CHECK: Original depositor from balance_account
    #[account(
        mut,
        constraint = depositor.key() == balance_account.depositor @ CustomError::InvalidDepositor
    )]
    pub depositor: AccountInfo<'info>, // ⭐ 원래 입금자 (rent 받을 사람)
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
    #[msg("Invalid original depositor.")]
    InvalidDepositor,
}