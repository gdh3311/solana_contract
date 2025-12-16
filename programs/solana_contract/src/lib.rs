use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash as sha256_hash;
use anchor_lang::solana_program::ed25519_program;
use anchor_lang::solana_program::sysvar::instructions::{load_instruction_at_checked, ID as IX_ID};

declare_id!("7EjvXnqXeniiXXSXCGC9UpeYXFwHSWmhWm2QDkqrxdJQ");

const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000;

#[program]
pub mod solana_contract {
    use super::*;
    
    pub fn deposit(
        ctx: Context<Deposit>,
        commitment: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);
        
        let acct = &mut ctx.accounts.commitment_account;
        acct.commitment = commitment;
        acct.deposited_amount = amount;
        acct.withdrawn_amount = 0;
        
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &acct.to_account_info().key(),
            amount,
        );
        
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[ctx.accounts.depositor.to_account_info(), acct.to_account_info()],
        )?;
        
        Ok(())
    }
    
    pub fn withdraw_zkp(
        ctx: Context<WithdrawZKP>,
        nonce: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        let acct = &mut ctx.accounts.commitment_account;
        let expected_h2 = acct.commitment;
        
        let challenge = create_withdrawal_challenge(
            &expected_h2,
            &ctx.accounts.recipient.key(),
            &nonce,
            amount,
        );
        
        verify_ed25519_signature(
            &ctx.accounts.ix_sysvar,
            &expected_h2,
            &challenge,
        )?;
        
        let remaining = acct
            .deposited_amount
            .checked_sub(acct.withdrawn_amount)
            .ok_or(ErrorCode::InsufficientBalance)?;
        
        require!(amount > 0, ErrorCode::InvalidAmount);
        
        let (actual_amount, is_full_withdrawal) = if amount >= remaining {
            (acct.to_account_info().lamports(), true)
        } else {
            (amount, false)
        };
        
        acct.withdrawn_amount = acct
            .withdrawn_amount
            .checked_add(if is_full_withdrawal { remaining } else { actual_amount })
            .ok_or(ErrorCode::Overflow)?;
        
        **acct.to_account_info().try_borrow_mut_lamports()? -= actual_amount;
        **ctx.accounts.recipient.try_borrow_mut_lamports()? += actual_amount;
        
        if is_full_withdrawal {
            acct.commitment = [0; 32];
            acct.deposited_amount = 0;
            acct.withdrawn_amount = 0;
        }
        
        Ok(())
    }
}

fn create_withdrawal_challenge(
    h2: &[u8; 32],
    recipient: &Pubkey,
    nonce: &[u8; 32],
    amount: u64,
) -> [u8; 32] {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(h2);
    data.extend_from_slice(recipient.as_ref());
    data.extend_from_slice(nonce);
    data.extend_from_slice(&amount.to_le_bytes());
    sha256_hash(&data).to_bytes()
}

fn verify_ed25519_signature(
    ix_sysvar: &AccountInfo,
    expected_pubkey: &[u8; 32],
    expected_message: &[u8; 32],
) -> Result<()> {
    require!(ix_sysvar.key == &IX_ID, ErrorCode::InvalidInstructionSysvar);
    
    let ix = load_instruction_at_checked(0, ix_sysvar)
        .map_err(|_| ErrorCode::Ed25519InstructionNotFound)?;
    
    require!(ix.program_id == ed25519_program::ID, ErrorCode::InvalidEd25519Instruction);
    
    let data = &ix.data;
    require!(data.len() >= 16 && data[0] == 1, ErrorCode::InvalidEd25519InstructionData);
    
    let sig_offset = u16::from_le_bytes([data[2], data[3]]) as usize;
    let pubkey_offset = u16::from_le_bytes([data[6], data[7]]) as usize;
    let msg_offset = u16::from_le_bytes([data[10], data[11]]) as usize;
    let msg_size = u16::from_le_bytes([data[12], data[13]]) as usize;
    
    require!(
        u16::from_le_bytes([data[4], data[5]]) == 0xFFFF &&
        u16::from_le_bytes([data[8], data[9]]) == 0xFFFF &&
        u16::from_le_bytes([data[14], data[15]]) == 0xFFFF &&
        msg_size == 32,
        ErrorCode::InvalidEd25519InstructionData
    );
    
    require!(
        sig_offset + 64 <= data.len() && 
        pubkey_offset + 32 <= data.len() && 
        msg_offset + 32 <= data.len(),
        ErrorCode::InvalidEd25519InstructionData
    );
    
    require!(
        &data[pubkey_offset..pubkey_offset + 32] == expected_pubkey,
        ErrorCode::PublicKeyMismatch
    );
    
    require!(
        &data[msg_offset..msg_offset + 32] == expected_message,
        ErrorCode::MessageMismatch
    );
    
    Ok(())
}

#[account]
pub struct CommitmentAccount {
    pub commitment: [u8; 32],
    pub deposited_amount: u64,
    pub withdrawn_amount: u64,
}

impl CommitmentAccount {
    pub const LEN: usize = 8 + 32 + 8 + 8;
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
pub struct WithdrawZKP<'info> {
    #[account(
        mut,
        seeds = [b"commitment", commitment_account.commitment.as_ref()],
        bump,
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,
    /// CHECK: Ed25519 verified
    #[account(mut)]
    pub recipient: AccountInfo<'info>,
    /// CHECK: Verified in instruction
    #[account(address = IX_ID)]
    pub ix_sysvar: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Amount too small")]
    AmountTooSmall,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Overflow")]
    Overflow,
    #[msg("Invalid instruction sysvar")]
    InvalidInstructionSysvar,
    #[msg("Ed25519 instruction not found")]
    Ed25519InstructionNotFound,
    #[msg("Invalid Ed25519 instruction")]
    InvalidEd25519Instruction,
    #[msg("Invalid Ed25519 data")]
    InvalidEd25519InstructionData,
    #[msg("Public key mismatch")]
    PublicKeyMismatch,
    #[msg("Message mismatch")]
    MessageMismatch,
}