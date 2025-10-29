use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash as keccak_hash;
use anchor_lang::solana_program::hash::hash as sha256_hash;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const COMMITMENT_DOMAIN: &[u8] = b"anonymous_pool_v1";
const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000; // 0.001 SOL

#[program]
pub mod anonymous_pool {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let vault = &mut ctx.accounts.pool_vault;
        vault.total_volume = 0;

        msg!("✅ PoolVault initialized");
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, commitment: [u8; 32], amount: u64) -> Result<()> {
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);

        let vault = &mut ctx.accounts.pool_vault;
        let commitment_account = &mut ctx.accounts.commitment_account;

        // Commitment metadata 설정
        commitment_account.commitment = commitment;
        commitment_account.amount = amount;

        // Deposit SOL → Vault PDA
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[ctx.accounts.depositor.to_account_info(), vault.to_account_info()],
        )?;

        vault.total_volume = vault.total_volume.saturating_add(amount);

        msg!("✅ Deposit complete: {} lamports", amount);
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, h1: [u8; 32]) -> Result<()> {
        let vault = &mut ctx.accounts.pool_vault;
        let commitment_account = &ctx.accounts.commitment_account;

        let computed_h2 = h2_from_h1(h1);
        require!(computed_h2 == commitment_account.commitment, ErrorCode::InvalidCommitment);

        let amount = commitment_account.amount;

        // Vault → recipient
        **vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.recipient.try_borrow_mut_lamports()? += amount;

        vault.total_volume = vault.total_volume.saturating_sub(amount);

        msg!("💸 Withdrawal complete: {} lamports", amount);
        Ok(())
    }

    pub fn get_pool_stats(ctx: Context<GetStats>) -> Result<()> {
        let vault = &ctx.accounts.pool_vault;
        msg!("📊 Vault total volume: {} lamports", vault.total_volume);
        Ok(())
    }
}

// Hash 함수
pub fn h2_from_h1(h1: [u8; 32]) -> [u8; 32] {
    let mut current = [COMMITMENT_DOMAIN, &h1[..]].concat();
    for _ in 0..5 {
        current = if current.len() % 2 == 0 {
            keccak_hash(&current).to_bytes().to_vec()
        } else {
            sha256_hash(&current).to_bytes().to_vec()
        };
    }
    current.try_into().expect("Hash output should be 32 bytes")
}

// Vault PDA 계정
#[account]
pub struct PoolVault {
    pub total_volume: u64, // 모든 SOL 합계
}

// Commitment 계정 - 실제 SOL 없음
#[account]
pub struct CommitmentAccount {
    pub commitment: [u8; 32],
    pub amount: u64, // Vault에서 관리되는 금액
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = admin, space = 8 + 8, seeds = [b"pool_vault"], bump)]
    pub pool_vault: Account<'info, PoolVault>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(commitment: [u8; 32])]
pub struct Deposit<'info> {
    #[account(mut, seeds = [b"pool_vault"], bump)]
    pub pool_vault: Account<'info, PoolVault>,
    #[account(init, payer = depositor, space = 8 + 32 + 8, seeds = [b"commitment", commitment.as_ref()], bump)]
    pub commitment_account: Account<'info, CommitmentAccount>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h1: [u8; 32])]
pub struct Withdraw<'info> {
    #[account(mut, seeds = [b"pool_vault"], bump)]
    pub pool_vault: Account<'info, PoolVault>,
    #[account(mut, seeds = [b"commitment", commitment_account.commitment.as_ref()], bump, close = recipient)]
    pub commitment_account: Account<'info, CommitmentAccount>,
    /// CHECK: Recipient of withdrawn funds
    #[account(mut)]
    pub recipient: AccountInfo<'info>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetStats<'info> {
    #[account(seeds = [b"pool_vault"], bump)]
    pub pool_vault: Account<'info, PoolVault>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid commitment - wrong H1 or commitment not found")]
    InvalidCommitment,
    #[msg("Deposit amount must be greater than 0")]
    InvalidAmount,
    #[msg("Minimum deposit is 0.001 SOL")]
    AmountTooSmall,
}
