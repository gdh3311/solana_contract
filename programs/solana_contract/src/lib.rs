use anchor_lang::prelude::*;

declare_id!("3NEr6ZiHYsW6eP2w6tk84yoVdWsRiDyYoe5qxY6qrTKL");

#[program]
pub mod solana_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.value = 0;
        state.message = "Hello Solana!".to_string();
        state.counter = 0;
        msg!("Initialized - value: {}, message: '{}'", state.value, state.message);
        Ok(())
    }

    pub fn set_value(ctx: Context<SetValue>, new_value: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.value = new_value;
        msg!("Updated value: {}", state.value);
        Ok(())
    }

    pub fn set_message(ctx: Context<SetValue>, new_message: String) -> Result<()> {
        let state = &mut ctx.accounts.state;
        require!(new_message.len() <= 50, CustomError::MessageTooLong);
        state.message = new_message.clone();
        msg!("Updated message: '{}'", new_message);
        Ok(())
    }

    pub fn increment_counter(ctx: Context<SetValue>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.counter = state.counter.checked_add(1).ok_or(CustomError::CounterOverflow)?;
        msg!("Counter incremented to: {}", state.counter);
        Ok(())
    }

    pub fn get_info(ctx: Context<GetInfo>) -> Result<StateInfo> {
        let state = &ctx.accounts.state;
        Ok(StateInfo {
            value: state.value,
            message: state.message.clone(),
            counter: state.counter,
            total_interactions: state.value + state.counter,
        })
    }
}

#[account]
pub struct StateAccount {
    pub value: u64,           // 8 bytes
    pub message: String,      // 4 + message length (최대 50자)
    pub counter: u64,         // 8 bytes
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init, 
        payer = user, 
        space = 8 + 8 + 4 + 50 + 8,
        seeds = [b"state", user.key().as_ref()], // PDA 시드 추가
        bump                                      // bump 추가
    )]
    pub state: Account<'info, StateAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetValue<'info> {
    #[account(
        mut,
        seeds = [b"state", user.key().as_ref()], // 같은 시드로 계정 찾기
        bump
    )]
    pub state: Account<'info, StateAccount>,
    pub user: Signer<'info>, // user 추가 (시드에 필요)
}

#[derive(Accounts)]
pub struct GetInfo<'info> {
    #[account(
        seeds = [b"state", user.key().as_ref()], // 같은 시드로 계정 찾기
        bump
    )]
    pub state: Account<'info, StateAccount>,
    pub user: Signer<'info>, // user 추가 (시드에 필요)
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct StateInfo {
    pub value: u64,
    pub message: String,
    pub counter: u64,
    pub total_interactions: u64,
}

#[error_code]
pub enum CustomError {
    #[msg("Message is too long. Maximum 50 characters allowed.")]
    MessageTooLong,
    #[msg("Counter overflow occurred.")]
    CounterOverflow,
}    