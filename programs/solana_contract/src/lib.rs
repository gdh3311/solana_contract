use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash as keccak_hash;
use anchor_lang::solana_program::hash::hash as sha256_hash;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const COMMITMENT_DOMAIN: &[u8] = b"anonymous_pool_v1";
const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000; // 0.001 SOL

#[program]
pub mod anonymous_pool {
    use super::*;

    // ✨ 첫 deposit 시 자동으로 Pool 생성 (init_if_needed)
    pub fn deposit(
        ctx: Context<Deposit>,
        commitment: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);

        let pool = &mut ctx.accounts.pool;
        let commitment_account = &mut ctx.accounts.commitment_account;

        // Commitment 설정
        commitment_account.commitment = commitment;
        commitment_account.amount = amount;

        // Pool 통계 업데이트
        pool.total_deposits += 1;
        pool.active_commitments += 1;
        pool.total_volume_deposited += amount;

        // SOL 전송
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &commitment_account.key(),
            amount,
        );

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.depositor.to_account_info(),
                commitment_account.to_account_info(),
            ],
        )?;

        msg!("✅ Deposit #{} complete", pool.total_deposits);
        msg!("   Amount: {} lamports ({:.4} SOL)", amount, amount as f64 / 1_000_000_000.0);
        
        // 첫 deposit인 경우 축하 메시지
        if pool.total_deposits == 1 {
            msg!("🎉 Pool automatically initialized with first deposit!");
        }
        
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, h1: [u8; 32]) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let commitment_account = &ctx.accounts.commitment_account;

        // H1 → H2 변환 및 검증
        let computed_h2 = h2_from_h1(h1);
        require!(
            computed_h2 == commitment_account.commitment,
            ErrorCode::InvalidCommitment
        );

        let withdraw_amount = commitment_account.amount;
        let total_lamports = commitment_account.to_account_info().lamports();

        // SOL 전송 및 commitment_account 닫기
        **commitment_account.to_account_info().try_borrow_mut_lamports()? = 0;
        **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? += total_lamports;

        // 통계 업데이트
        pool.total_withdrawals += 1;
        pool.active_commitments = pool.active_commitments.saturating_sub(1);
        pool.total_volume_withdrawn += withdraw_amount;

        msg!("💸 Withdrawal complete");
        msg!("   Amount: {} lamports", withdraw_amount);
        msg!("   Total received (with rent): {} lamports", total_lamports);

        Ok(())
    }

    pub fn get_pool_stats(ctx: Context<GetStats>) -> Result<()> {
        let pool = &ctx.accounts.pool;

        msg!("📊 Pool Statistics:");
        msg!("   Total Deposits: {}", pool.total_deposits);
        msg!("   Total Withdrawals: {}", pool.total_withdrawals);
        msg!("   Active Commitments: {}", pool.active_commitments);
        msg!("   Total Volume Deposited: {} SOL", pool.total_volume_deposited / 1_000_000_000);
        msg!("   Total Volume Withdrawn: {} SOL", pool.total_volume_withdrawn / 1_000_000_000);
        
        if pool.total_deposits > 0 {
            msg!("   Pool Efficiency: {:.2}%", 
                (pool.total_withdrawals as f64 / pool.total_deposits as f64) * 100.0
            );
        }
        
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

// Account 구조체
#[account]
#[derive(Default)]  // Default trait 추가로 자동 초기화
pub struct Pool {
    pub total_deposits: u64,
    pub total_withdrawals: u64,
    pub active_commitments: u64,
    pub total_volume_deposited: u64,
    pub total_volume_withdrawn: u64,
}

#[account]
pub struct CommitmentAccount {
    pub commitment: [u8; 32],
    pub amount: u64,
}

// ✨ init_if_needed 사용 - Pool이 없으면 자동 생성
#[derive(Accounts)]
#[instruction(commitment: [u8; 32])]
pub struct Deposit<'info> {
    #[account(
        init_if_needed,  // 👈 핵심: 필요시 자동 생성
        payer = depositor,
        space = 8 + 40,
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        init,
        payer = depositor,
        space = 8 + 32 + 8,
        seeds = [b"commitment", commitment.as_ref()],
        bump
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,
    
    #[account(mut)]
    pub depositor: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h1: [u8; 32])]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>,
    
    #[account(
        mut,
        seeds = [b"commitment", commitment_account.commitment.as_ref()],
        bump,
        close = recipient
    )]
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
    #[account(
        seeds = [b"pool"],
        bump
    )]
    pub pool: Account<'info, Pool>,
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