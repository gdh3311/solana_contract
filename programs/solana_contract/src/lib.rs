use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash as keccak_hash;
use anchor_lang::solana_program::hash::hash as sha256_hash;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const COMMITMENT_DOMAIN: &[u8] = b"anonymous_pool_v1";
const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000;

#[program]
pub mod anonymous_pool {
    use super::*;

    // deposit: sender provides commitment = h2, amount, and nullifier (keccak(h1||recipient))
    pub fn deposit(ctx: Context<Deposit>, commitment: [u8;32], amount: u64, nullifier: [u8;32]) -> Result<()> {
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);
        let acct = &mut ctx.accounts.commitment_account;
        acct.commitment = commitment;
        acct.amount = amount;
        acct.nullifier = nullifier;
        acct.nullifier_used = false;

        // transfer lamports into PDA
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &acct.to_account_info().key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[ctx.accounts.depositor.to_account_info(), acct.to_account_info()],
        )?;
        msg!("deposit stored");
        Ok(())
    }

    // withdraw: recipient calls with h1 only
pub fn withdraw(ctx: Context<Withdraw>, h1: [u8;32]) -> Result<()> {
        let acct = &mut ctx.accounts.commitment_account;
        require!(!acct.nullifier_used, ErrorCode::NullifierAlreadyUsed);

        // 1) PDA match already enforced by account constraint (seeds = [b"commitment", h2_from_h1(h1)])
        // 2) compute expected nullifier = keccak(h1 || recipient)
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(&h1);
        buf.extend_from_slice(ctx.accounts.recipient.key.as_ref());
        let expected_nullifier = keccak_hash(&buf).to_bytes();
        require!(expected_nullifier == acct.nullifier, ErrorCode::InvalidNullifier);

        acct.nullifier_used = true;

        // transfer lamports out and close
        let total = acct.to_account_info().lamports();
        **acct.to_account_info().try_borrow_mut_lamports()? = 0;
        **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? += total;

        msg!("withdraw success: {} lamports", acct.amount);
        Ok(())
    }
}

// simple h2 derivation (same as your chain of hashes)
pub fn h2_from_h1(h1: [u8;32]) -> [u8;32] {
    let mut current = [COMMITMENT_DOMAIN, &h1[..]].concat();
    current = sha256_hash(&current).to_bytes().to_vec();
    current = keccak_hash(&current).to_bytes().to_vec();
    current = sha256_hash(&current).to_bytes().to_vec();
    current.try_into().expect("32 bytes")
}

#[account]
pub struct CommitmentAccount {
    pub commitment: [u8;32],
    pub amount: u64,
    pub nullifier: [u8;32],
    pub nullifier_used: bool,
}

impl CommitmentAccount { pub const LEN: usize = 8+32+8+32+1; }

#[derive(Accounts)]
#[instruction(commitment: [u8;32])]
pub struct Deposit<'info> {
    #[account(init, payer=depositor, space = CommitmentAccount::LEN, seeds=[b"commitment", commitment.as_ref()], bump)]
    pub commitment_account: Account<'info, CommitmentAccount>,
    #[account(mut)] pub depositor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(h1: [u8;32])]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds=[b"commitment", h2_from_h1(h1).as_ref()],
        bump,
        close = recipient
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,
    /// CHECK: any recipient is allowed but nullifier ties to recipient
    #[account(mut)]
    pub recipient: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}


#[error_code]
pub enum ErrorCode {
    #[msg("Amount too small")] AmountTooSmall,
    #[msg("Nullifier already used")] NullifierAlreadyUsed,
    #[msg("Invalid nullifier - recipient mismatch")] InvalidNullifier,
}
