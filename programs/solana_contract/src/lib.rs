use anchor_lang::prelude::*;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

#[program]
pub mod solana_contract {
    use super::*;

    // mapping에 값 저장 (입금)
    pub fn deposit(ctx: Context<Deposit>, hash_key: String, amount: u64) -> Result<()> {
        require!(hash_key.len() <= 32, CustomError::KeyTooLong);
        require!(amount > 0, CustomError::InvalidAmount);
        
        let balance_account = &mut ctx.accounts.balance_account;
        balance_account.hash_key = hash_key.clone();
        balance_account.balance = amount;

        // SOL 전송
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

    // mapping에서 값 가져오기 (출금)
    pub fn withdraw(ctx: Context<Withdraw>, hash_key: String) -> Result<()> {
        let balance_account = &ctx.accounts.balance_account;
        
        require!(balance_account.hash_key == hash_key, CustomError::InvalidKey);
        require!(balance_account.balance > 0, CustomError::NoBalance);

        let amount = balance_account.balance;

        // SOL 전송
        **ctx.accounts.balance_account.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount;

        msg!("Withdrawn {} lamports with key: {}", amount, hash_key);
        Ok(())
    }
}

// mapping의 value 구조체
#[account]
pub struct BalanceAccount {
    pub hash_key: String,
    pub balance: u64,
}

// deposit: balances[hash] = amount
#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 4 + 32 + 8,
        seeds = [b"balance", hash_key.as_bytes()],  // mapping의 key
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// withdraw: amount = balances[hash]
#[derive(Accounts)]
#[instruction(hash_key: String)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        close = user,
        seeds = [b"balance", hash_key.as_bytes()],  // mapping의 key로 찾기
        bump
    )]
    pub balance_account: Account<'info, BalanceAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
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
}