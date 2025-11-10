use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash as keccak_hash;
use anchor_lang::solana_program::hash::hash as sha256_hash;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const COMMITMENT_DOMAIN: &[u8] = b"anonymous_pool_v1";
const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000;

#[program]
pub mod anonymous_pool {
    use super::*;

    pub fn deposit(
        ctx: Context<Deposit>,
        commitment: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);

        let acct = &mut ctx.accounts.commitment_account;
        acct.commitment = commitment;
        acct.amount = amount;
        acct.withdrawn = false;

        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &acct.to_account_info().key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[ctx.accounts.depositor.to_account_info(), acct.to_account_info()],
        )?;

        msg!("✅ deposit: {} lamports", amount);
        Ok(())
    }

    pub fn withdraw_commit(
        ctx: Context<WithdrawCommit>,
        withdraw_commitment: [u8; 32],
    ) -> Result<()> {
        let withdraw_state = &mut ctx.accounts.withdraw_state;
        
        withdraw_state.withdraw_commitment = withdraw_commitment;
        withdraw_state.commitment_account = ctx.accounts.commitment_account.key();
        withdraw_state.revealed = false;

        msg!("🔒 Withdraw committed");
        Ok(())
    }

    pub fn withdraw_reveal(
        ctx: Context<WithdrawReveal>,
        h1: [u8; 32],
        recipient: Pubkey,
        salt: [u8; 32],
    ) -> Result<()> {
        let acct = &mut ctx.accounts.commitment_account;
        let withdraw_state = &mut ctx.accounts.withdraw_state;

        let computed_h2 = h2_from_h1(h1);
        require!(computed_h2 == acct.commitment, ErrorCode::InvalidCommitment);

        let computed_withdraw_commitment = compute_withdraw_commitment(&computed_h2, &recipient, &salt);
        require!(
            computed_withdraw_commitment == withdraw_state.withdraw_commitment,
            ErrorCode::InvalidReveal
        );

        require!(!withdraw_state.revealed, ErrorCode::AlreadyRevealed);
        require!(!acct.withdrawn, ErrorCode::AlreadyWithdrawn);

        acct.withdrawn = true;
        withdraw_state.revealed = true;

        let total = acct.to_account_info().lamports();
        **acct.to_account_info().try_borrow_mut_lamports()? = 0;
        **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? += total;

        msg!("💸 withdraw: {} lamports → {}", total, recipient);
        Ok(())
    }
}

fn compute_withdraw_commitment(h2: &[u8; 32], recipient: &Pubkey, salt: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(96);
    buf.extend_from_slice(h2);
    buf.extend_from_slice(recipient.as_ref());
    buf.extend_from_slice(salt);
    keccak_hash(&buf).to_bytes()
}

pub fn h2_from_h1(h1: [u8; 32]) -> [u8; 32] {
    let mut current = [COMMITMENT_DOMAIN, &h1[..]].concat();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current.try_into().expect("Hash output should be 32 bytes")
}

#[account]
pub struct CommitmentAccount {
    pub commitment: [u8; 32],
    pub amount: u64,
    pub withdrawn: bool,
}
impl CommitmentAccount {
    pub const LEN: usize = 8 + 32 + 8 + 1;
}

#[account]
pub struct WithdrawState {
    pub withdraw_commitment: [u8; 32],
    pub commitment_account: Pubkey,
    pub revealed: bool,
}
impl WithdrawState {
    pub const LEN: usize = 8 + 32 + 32 + 1;
}

#[derive(Accounts)]
#[instruction(commitment: [u8; 32])]
pub struct Deposit<'info> {
    #[account(
        init,
        payer = depositor,
        space = CommitmentAccount::LEN,
        seeds = [b"commitment", commitment.as_ref()],
        bump
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(withdraw_commitment: [u8; 32])]
pub struct WithdrawCommit<'info> {
    #[account(
        constraint = !commitment_account.withdrawn @ ErrorCode::AlreadyWithdrawn
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,

    #[account(
        init,
        payer = fee_payer,
        space = WithdrawState::LEN,
        seeds = [b"withdraw", withdraw_commitment.as_ref()],
        bump
    )]
    pub withdraw_state: Account<'info, WithdrawState>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct WithdrawReveal<'info> {
    #[account(
        mut,
        seeds = [b"commitment", commitment_account.commitment.as_ref()],
        bump,
        close = recipient
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,

    #[account(
        mut,
        seeds = [b"withdraw", withdraw_state.withdraw_commitment.as_ref()],
        bump,
        close = recipient
    )]
    pub withdraw_state: Account<'info, WithdrawState>,

    #[account(mut)]
    pub recipient: Signer<'info>, 

    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid commitment")]
    InvalidCommitment,
    #[msg("Already withdrawn")]
    AlreadyWithdrawn,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Amount too small")]
    AmountTooSmall,
    #[msg("Invalid reveal")]
    InvalidReveal,
    #[msg("Already revealed")]
    AlreadyRevealed,
}