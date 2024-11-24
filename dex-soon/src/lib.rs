use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("7JP7E2jTjUeQadvTPXJdxxrz73SBi92GzEbTK6yerHGq");

fn calculate_lp_tokens(amount_a: u64, amount_b: u64, pool: &Pool) -> Result<u64> {
    if pool.lp_supply == 0 {
        let product = (amount_a as f64) * (amount_b as f64);
        let sqrt_product = (product as f64).sqrt();
        Ok(sqrt_product as u64)
    } else {
        let lp_from_a = (amount_a as u128 * pool.lp_supply as u128) / pool.token_a_balance as u128;
        let lp_from_b = (amount_b as u128 * pool.lp_supply as u128) / pool.token_b_balance as u128;
        Ok(lp_from_a.min(lp_from_b) as u64)
    }
}

#[program]
pub mod my_dex {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.token_a_balance = 0;
        pool.token_b_balance = 0;
        pool.lp_supply = 0;
        Ok(())
    }

    pub fn provide_liquidity(
        ctx: Context<ProvideLiquidity>,
        amount_a: u64,
        amount_b: u64,
    ) -> Result<()> {
        // First perform the transfers
        token::transfer(ctx.accounts.transfer_a_context(), amount_a)?;
        token::transfer(ctx.accounts.transfer_b_context(), amount_b)?;

        // Then update the pool state
        let pool = &mut ctx.accounts.pool;
        pool.token_a_balance += amount_a;
        pool.token_b_balance += amount_b;

        // Calculate and mint LP tokens
        let lp_tokens = calculate_lp_tokens(amount_a, amount_b, pool)?;
        pool.lp_supply += lp_tokens;

        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, is_token_a_to_b: bool) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        // Calculate amounts before modifying the pool
        let (reserve_in, reserve_out) = if is_token_a_to_b {
            (pool.token_a_balance, pool.token_b_balance)
        } else {
            (pool.token_b_balance, pool.token_a_balance)
        };

        let amount_out = calculate_output(amount_in, reserve_in, reserve_out)?;

        // Update pool balances
        if is_token_a_to_b {
            pool.token_a_balance += amount_in;
            pool.token_b_balance -= amount_out;
        } else {
            pool.token_b_balance += amount_in;
            pool.token_a_balance -= amount_out;
        }

        // Perform the transfer
        token::transfer(ctx.accounts.swap_context(), amount_out)?;

        Ok(())
    }
}

#[account]
pub struct Pool {
    pub token_a_balance: u64,
    pub token_b_balance: u64,
    pub lp_supply: u64,
}

fn calculate_output(amount_in: u64, reserve_in: u64, reserve_out: u64) -> Result<u64> {
    let amount_in_with_fee = amount_in * 997; // 0.3% fee
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in * 1000 + amount_in_with_fee;
    Ok(numerator / denominator)
}

// Rest of the structs and implementations remain the same...
#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(init, payer = user, space = 8 + 32)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ProvideLiquidity<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token_b: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_a: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token_b: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> ProvideLiquidity<'info> {
    pub fn transfer_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.user_token_a.to_account_info(),
            to: self.pool_token_a.to_account_info(),
            authority: self.user.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }

    pub fn transfer_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.user_token_b.to_account_info(),
            to: self.pool_token_b.to_account_info(),
            authority: self.user.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    #[account(mut)]
    pub pool_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Swap<'info> {
    pub fn swap_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.pool_token.to_account_info(),
            to: self.user_token.to_account_info(),
            authority: self.pool.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}
