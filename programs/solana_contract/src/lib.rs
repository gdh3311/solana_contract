use anchor_lang::prelude::*;

declare_id!("6VmnbE29VNhm3qgRtfDv2dKyCSMdT7wDFdV9bobjyiAm");

#[program]
pub mod solana_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.state.value = 0;
        msg!("Initialized value: {}", ctx.accounts.state.value);
        Ok(())
    }

    pub fn set_value(ctx: Context<SetValue>, new_value: u64) -> Result<()> {
        ctx.accounts.state.value = new_value;
        msg!("Updated value: {}", ctx.accounts.state.value);
        Ok(())
    }
}

#[account]
pub struct StateAccount {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 16)]
    pub state: Account<'info, StateAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetValue<'info> {
    #[account(mut)]
    pub state: Account<'info, StateAccount>,
}
