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
    
    // Create challenge and verify signature
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
    
    // Calculate remaining balance
    let remaining = acct
        .deposited_amount
        .checked_sub(acct.withdrawn_amount)
        .ok_or(ErrorCode::InsufficientBalance)?;
    
    // Validate requested amount
    require!(amount > 0, ErrorCode::InvalidAmount);
    
    // 🔥 전액 출금 로직: 요청 금액이 잔액 이상이면 전부 + rent 반환
    let (actual_amount, is_full_withdrawal) = if amount >= remaining {
        // 전액 출금: PDA의 모든 lamports (잔액 + rent)
        let total_lamports = acct.to_account_info().lamports();
        (total_lamports, true)
    } else {
        // 부분 출금: 요청한 만큼만
        (amount, false)
    };
    
    // Update state
    acct.withdrawn_amount = acct
        .withdrawn_amount
        .checked_add(if is_full_withdrawal { remaining } else { actual_amount })
        .ok_or(ErrorCode::Overflow)?;
    
    // Transfer SOL
    **acct.to_account_info().try_borrow_mut_lamports()? -= actual_amount;
    **ctx.accounts.recipient.try_borrow_mut_lamports()? += actual_amount;
    
    msg!("✅ ZKP Withdrawal: {} lamports", actual_amount);
    msg!("   Deposited: {}, Withdrawn: {}, Remaining: {}", 
         acct.deposited_amount, 
         acct.withdrawn_amount,
         acct.deposited_amount - acct.withdrawn_amount);
    
    if is_full_withdrawal {
        msg!("🔒 Full withdrawal - closing account and returning rent");
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


// challenge = hash(H2 + recipient + nonce + amount)
//          = hash([공개] + [공개] + [공개] + [공개])
// ```

// **모든 입력값이 공개 정보!**
// - H2: 공개 (온체인에 저장)
// - recipient: 공개 (트랜잭션에 명시)
// - nonce: 공개 (트랜잭션에 명시)
// - amount: 공개 (트랜잭션에 명시)

// **Challenge도 공개 정보!** (Ed25519 instruction의 message에 포함)

// ### 3. 비밀은 **서명**에 있음
// ```
// signature = sign(H1, challenge)
//               ↑
//            비밀키!
// ```

// **보안의 핵심:**
// - **H1 (비밀키)을 알아야만** 유효한 서명 생성 가능
// - Nonce, challenge를 알아도 **H1 없이는 서명 불가능**

// ## 공격 시나리오 분석

// ### 공격 1: C가 B의 nonce를 그대로 사용
// ```
// C가 시도:
// challenge_C = hash(H2 + C_address + nonce_B + 0.25)
// signature_C = sign(H1, challenge_C)
// ```

// **결과: ✅ 성공!**

// **하지만 문제없는 이유:**
// - C는 H1을 원래 알고 있음 (A가 줬으니까)
// - C는 **자기 몫(0.25)**만 출금
// - B의 몫(0.5)은 여전히 안전
// ```
// 실행 순서:
// 1. C가 B의 nonce 사용해서 0.25 출금 → 성공
// 2. B가 자기 nonce 사용해서 0.5 출금 → 성공
// ```

// **왜 안전?** Challenge에 **recipient와 amount가 포함**되어 있어서!

// ### 공격 2: C가 B의 서명을 복사
// ```
// C가 B의 트랜잭션에서 복사:
// - signature_B
// - nonce_B
// - amount: 0.5

// C가 recipient만 자기 주소로 바꿔서 실행 시도