use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash as sha256_hash;
use anchor_lang::solana_program::ed25519_program;
use anchor_lang::solana_program::sysvar::instructions::{load_instruction_at_checked, ID as IX_ID};

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

const MIN_DEPOSIT_AMOUNT: u64 = 1_000_000;

#[program]
pub mod solana_contract {
    use super::*;
    
    /// Deposit funds with h2 commitment
    pub fn deposit(
        ctx: Context<Deposit>,
        commitment: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, ErrorCode::InvalidAmount);
        require!(amount >= MIN_DEPOSIT_AMOUNT, ErrorCode::AmountTooSmall);
        
        let acct = &mut ctx.accounts.commitment_account;
        acct.commitment = commitment;
        acct.deposited_amount = amount;
        acct.withdrawn_amount = 0;
        
        // Transfer SOL to PDA
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.depositor.key(),
            &acct.to_account_info().key(),
            amount,
        );
        
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[ctx.accounts.depositor.to_account_info(), acct.to_account_info()],
        )?;
        
        msg!("✅ Deposit: {} lamports, h2: {:?}", amount, commitment);
        Ok(())
    }
    
    /// Direct withdrawal using ZK proof (no commit-reveal needed)
    /// H1 is NEVER exposed on-chain!
    pub fn withdraw_zkp(
        ctx: Context<WithdrawZKP>,
        nonce: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        let acct = &mut ctx.accounts.commitment_account;
        let expected_h2 = acct.commitment;
        
        // Create challenge message = hash(h2 + recipient + nonce + amount)
        let challenge = create_withdrawal_challenge(
            &expected_h2,
            &ctx.accounts.recipient.key(),
            &nonce,
            amount,
        );
        
        // Verify the Ed25519 signature
        // Ed25519 프로그램이 이미 서명을 검증했는지 확인
        // instruction 0번에 Ed25519Verify가 있어야 함
        verify_ed25519_signature(
            &ctx.accounts.ix_sysvar,
            &expected_h2,
            &challenge,
        )?;
        
        // Calculate withdrawal amount
        let remaining = acct
            .deposited_amount
            .checked_sub(acct.withdrawn_amount)
            .ok_or(ErrorCode::InsufficientBalance)?;
        
        let requested_amount = amount.min(remaining);
        require!(requested_amount > 0, ErrorCode::InvalidAmount);
        
        // Handle full vs partial withdrawal
        let actual_amount = if requested_amount == remaining {
            // Full withdrawal - take everything
            acct.to_account_info().lamports()
        } else {
            // Partial withdrawal - respect rent exemption
            let pda_balance = acct.to_account_info().lamports();
            let rent_exempt = Rent::get()?.minimum_balance(CommitmentAccount::LEN);
            let available = pda_balance.checked_sub(rent_exempt).unwrap_or(0);
            
            require!(requested_amount <= available, ErrorCode::InsufficientBalance);
            requested_amount
        };
        
        // Update state
        acct.withdrawn_amount = acct
            .withdrawn_amount
            .checked_add(actual_amount)
            .ok_or(ErrorCode::Overflow)?;
        
        // Transfer SOL
        **acct.to_account_info().try_borrow_mut_lamports()? -= actual_amount;
        **ctx.accounts.recipient.try_borrow_mut_lamports()? += actual_amount;
        
        msg!("✅ ZKP Withdrawal successful: {} lamports", actual_amount);
        msg!("🔐 H1 was NEVER exposed on-chain!");
        
        // Close account if fully withdrawn
        if acct.deposited_amount == acct.withdrawn_amount {
            msg!("Closing empty commitment account");
        }
        
        Ok(())
    }
}

/// Create challenge for withdrawal proof
fn create_withdrawal_challenge(
    h2: &[u8; 32],
    recipient: &Pubkey,
    nonce: &[u8; 32],
    amount: u64,
) -> [u8; 32] {
    let mut data = Vec::with_capacity(32 + 32 + 32 + 8);
    data.extend_from_slice(h2);
    data.extend_from_slice(recipient.as_ref());
    data.extend_from_slice(nonce);
    data.extend_from_slice(&amount.to_le_bytes());
    
    sha256_hash(&data).to_bytes()
}

/// Verify Ed25519 signature proof
/// Ed25519 프로그램이 instruction 0번에서 이미 검증을 완료했는지 확인
fn verify_ed25519_signature(
    ix_sysvar: &AccountInfo,
    expected_pubkey: &[u8; 32],
    expected_message: &[u8; 32],
) -> Result<()> {
    require!(
        ix_sysvar.key == &IX_ID,
        ErrorCode::InvalidInstructionSysvar
    );
    
    let ix = load_instruction_at_checked(0, ix_sysvar)
        .map_err(|_| ErrorCode::Ed25519InstructionNotFound)?;
    
    require!(
        ix.program_id == ed25519_program::ID,
        ErrorCode::InvalidEd25519Instruction
    );
    
    let data = &ix.data;
    require!(data.len() >= 16, ErrorCode::InvalidEd25519InstructionData); // 16 not 17!
    
    let num_signatures = data[0];
    require!(num_signatures == 1, ErrorCode::InvalidEd25519InstructionData);
    
    // ✅ 수정: 1-byte padding 기준으로 파싱
    // Header: [0]=num_sigs, [1]=padding, [2-15]=offsets
    let sig_offset = u16::from_le_bytes([data[2], data[3]]) as usize;
    let sig_ix_index = u16::from_le_bytes([data[4], data[5]]);
    
    let pubkey_offset = u16::from_le_bytes([data[6], data[7]]) as usize;
    let pubkey_ix_index = u16::from_le_bytes([data[8], data[9]]);
    
    let msg_offset = u16::from_le_bytes([data[10], data[11]]) as usize;
    let msg_size = u16::from_le_bytes([data[12], data[13]]) as usize;
    let msg_ix_index = u16::from_le_bytes([data[14], data[15]]);
    
    msg!("Parsed Ed25519 instruction:");
    msg!("  sig_ix_index: 0x{:04x}", sig_ix_index);
    msg!("  pubkey_ix_index: 0x{:04x}", pubkey_ix_index);
    msg!("  msg_ix_index: 0x{:04x}", msg_ix_index);
    
    // All data should be in the Ed25519 instruction itself
    require!(
        sig_ix_index == 0xFFFF && pubkey_ix_index == 0xFFFF && msg_ix_index == 0xFFFF,
        ErrorCode::InvalidEd25519InstructionData
    );
    
    // Verify message size
    require!(msg_size == 32, ErrorCode::InvalidMessageSize);
    
    // Extract actual data from instruction
    let sig_end = sig_offset + 64;
    let pubkey_end = pubkey_offset + 32;
    let msg_end = msg_offset + msg_size;
    
    require!(
        sig_end <= data.len() && pubkey_end <= data.len() && msg_end <= data.len(),
        ErrorCode::InvalidEd25519InstructionData
    );
    
    let actual_pubkey = &data[pubkey_offset..pubkey_end];
    let actual_message = &data[msg_offset..msg_end];
    
    // Verify public key matches H2
    require!(
        actual_pubkey == expected_pubkey,
        ErrorCode::PublicKeyMismatch
    );
    
    // Verify message matches challenge
    require!(
        actual_message == expected_message,
        ErrorCode::MessageMismatch
    );
    
    msg!("✅ ZK Proof verified - signer knows H1 without revealing it!");
    msg!("   H2 (pubkey): {:?}", expected_pubkey);
    msg!("   Challenge: {:?}", expected_message);
    
    Ok(())
}

// ============================================================================
// Account Structures
// ============================================================================

#[account]
pub struct CommitmentAccount {
    pub commitment: [u8; 32],      // H2 (public commitment)
    pub deposited_amount: u64,
    pub withdrawn_amount: u64,
}

impl CommitmentAccount {
    pub const LEN: usize = 8 + 32 + 8 + 8;
}

// ============================================================================
// Account Contexts
// ============================================================================

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
        close = recipient
    )]
    pub commitment_account: Account<'info, CommitmentAccount>,
    
    #[account(mut)]
    pub recipient: Signer<'info>,
    
    /// Instruction sysvar for Ed25519 verification
    /// CHECK: Verified in instruction
    #[account(address = IX_ID)]
    pub ix_sysvar: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

// ============================================================================
// Error Codes
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Amount too small - minimum 0.001 SOL")]
    AmountTooSmall,
    
    #[msg("Insufficient balance")]
    InsufficientBalance,
    
    #[msg("Arithmetic overflow")]
    Overflow,
    
    #[msg("Invalid ZK proof - failed to verify knowledge of H1")]
    InvalidProof,
    
    #[msg("Invalid instruction sysvar")]
    InvalidInstructionSysvar,
    
    #[msg("Ed25519 instruction not found")]
    Ed25519InstructionNotFound,
    
    #[msg("Invalid Ed25519 instruction")]
    InvalidEd25519Instruction,
    
    #[msg("Invalid Ed25519 instruction data format")]
    InvalidEd25519InstructionData,
    
    #[msg("Signature mismatch")]
    SignatureMismatch,
    
    #[msg("Public key mismatch")]
    PublicKeyMismatch,
    
    #[msg("Message mismatch")]
    MessageMismatch,
    
    #[msg("Invalid message size")]
    InvalidMessageSize,
}