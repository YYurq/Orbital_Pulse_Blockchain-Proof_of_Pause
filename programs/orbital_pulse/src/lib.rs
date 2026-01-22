use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::sysvar::slot_hashes;

declare_id!("3o6We5WQoGDM6wpQMPq5VE3fjvC7zgCUD56X12vLn917");

#[program]
pub mod orbital_pulse {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, epsilon: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.signer.key();
        state.epsilon = epsilon;
        state.last_noise = 0;
        state.current_orbit = 0;
        state.current_depth = 11;
        state.head = 0;
        state.variance_index = 0;
        state.prev_variance_index = 0;
        state.last_fine_log = 0;
        Ok(())
    }

    pub fn try_transition(ctx: Context<Transition>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        
        // 1. Извлечение энтропии
        let data = ctx.accounts.slot_hashes.try_borrow_data()?;
        let recent_hash: [u8; 32] = data[12..44].try_into().map_err(|_| ErrorCode::HashNotFound)?;
        let noise_hash = hashv(&[&recent_hash, &state.authority.to_bytes()]);
        let noise = u64::from_le_bytes(noise_hash.as_ref()[0..8].try_into().unwrap());
        
        let delta = if noise > state.last_noise { noise - state.last_noise } else { state.last_noise - noise };

        // 2. Спектральный след (локальная копия индекса для Borrow Checker)
        let h_idx = state.head as usize;
        state.history[h_idx] = delta;
        state.head = (state.head + 1) % 16;

        // 3. Вычисление мгновенной дисперсии
        let mut sum: u128 = 0;
        let depth = state.current_depth as usize;
        let current_head = state.head as usize;

        for i in 0..depth {
            let idx = (current_head + 16 - 1 - i) % 16;
            sum += state.history[idx] as u128;
        }
        let avg = sum / depth as u128;
        
        let mut variance_sum: u128 = 0;
        for i in 0..depth {
            let idx = (current_head + 16 - 1 - i) % 16;
            let val = state.history[idx] as u128;
            let diff = if val > avg { val - avg } else { avg - val };
            variance_sum = variance_sum.saturating_add(diff * diff);
        }

        // 4. Логарифмическая нормализация (Fine Log)
        let mean_variance = variance_sum / depth as u128;
        let msb = 128 - mean_variance.leading_zeros() as i32;
        
        let fine_log = if msb > 8 {
            let integer_part = (msb as u64) << 8; 
            let fractional_part = ((mean_variance >> (msb - 8)) & 0xFF) as u64;
            integer_part + fractional_part
        } else if msb > 0 { (msb as u64) << 8 } else { 0 };

        state.last_fine_log = fine_log;

        // 5. EMA и Адаптивная глубина
        state.prev_variance_index = state.variance_index;
        state.variance_index = ((state.variance_index as f64 * 0.8) + (fine_log as f64 * 0.2)) as u64;
        
        let gradient = state.variance_index as i64 - state.prev_variance_index as i64;
        if gradient < 0 && state.current_depth > 7 { state.current_depth -= 1; }
        else if gradient > 0 && state.current_depth < 15 { state.current_depth += 1; }

        // 6. Условие Резонанса
        if delta < state.epsilon && state.variance_index < (state.epsilon.saturating_mul(10)) {
            state.current_orbit = (state.current_orbit + 1) % 5;
            let bump = ctx.bumps.mint;
            let seeds = &[b"orbital-genesis".as_ref(), &[bump]];
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.token_account.to_account_info(),
                        authority: ctx.accounts.mint.to_account_info(),
                    },
                    &[&seeds[..]],
                ),
                100_000_000 
            )?;
            state.epsilon = state.epsilon.saturating_sub(state.epsilon / 100);
        }

        state.last_noise = noise;
        Ok(())
    }
}

#[account]
pub struct PulseState {
    pub authority: Pubkey,        
    pub last_noise: u64,          
    pub epsilon: u64,             
    pub current_orbit: u8,        
    pub history: [u64; 16],       
    pub current_depth: u8,        
    pub head: u8,                 
    pub variance_index: u64,      
    pub prev_variance_index: u64, 
    pub last_fine_log: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = signer, space = 8 + 211)]
    pub state: Account<'info, PulseState>,
    #[account(init_if_needed, payer = signer, mint::decimals = 9, mint::authority = mint, seeds = [b"orbital-genesis"], bump)]
    pub mint: Account<'info, Mint>,
    #[account(init_if_needed, payer = signer, associated_token::mint = mint, associated_token::authority = signer)]
    pub token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Transition<'info> {
    #[account(mut)]
    pub state: Account<'info, PulseState>,
    #[account(mut, seeds = [b"orbital-genesis"], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    #[account(address = slot_hashes::ID)]
    pub slot_hashes: AccountInfo<'info>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Хеш не найден")] HashNotFound,
}
