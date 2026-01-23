use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
use anchor_lang::solana_program::hash::hashv;
use anchor_lang::solana_program::sysvar::slot_hashes;

declare_id!("3o6We5WQoGDM6wpQMPq5VE3fjvC7zgCUD56X12vLn917");

const CALIBRATION_STEPS: u8 = 16; 

#[program]
pub mod orbital_pulse {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, threshold_percent: u64) -> Result<()> {
        let state = &mut ctx.accounts.state;
        state.authority = ctx.accounts.signer.key();
        
        state.history = [0u64; 16]; 
        state.calib_count = 0;
        state.is_born = false;
        
        state.gradient_threshold_percent = threshold_percent.clamp(1, 10);
        state.mode = 255; // Режим CALIBRATION
        state.epsilon = 0;
        state.x_control = 0;
        state.current_depth = 11;
        state.head = 0;
        state.variance_index = 0;
        state.prev_variance_index = 0;
        state.last_noise = 0;
        Ok(())
    }

    pub fn try_transition(ctx: Context<Transition>) -> Result<()> {
        let state = &mut ctx.accounts.state;
        
        // 1. Извлечение энтропии
        let data = ctx.accounts.slot_hashes.try_borrow_data()?;
        let hash: [u8; 32] = data[12..44].try_into().map_err(|_| ErrorCode::HashNotFound)?;
        let n_hash = hashv(&[&hash, &state.authority.to_bytes()]);
        let noise = u64::from_le_bytes(n_hash.as_ref()[0..8].try_into().unwrap());
        let delta = if noise > state.last_noise { noise - state.last_noise } else { state.last_noise - noise };

        // 2. ФАЗА РОЖДЕНИЯ (Медианная калибровка)
        if !state.is_born {
            let idx = state.calib_count as usize; 
            state.history[idx] = delta;
            state.calib_count += 1;

            if state.calib_count == CALIBRATION_STEPS {
                let mut h = state.history;
                h[0..CALIBRATION_STEPS as usize].sort();
                // Медиана — устойчивый физический вакуум
                state.epsilon = h[CALIBRATION_STEPS as usize / 2]; 
                state.is_born = true;
                state.mode = 0; // S0: Aligned
            }
            state.last_noise = noise;
            return Ok(());
        }

        // 3. РАБОЧИЙ ЦИКЛ (S0, S1, S2)
        let h_idx = state.head as usize;
        state.history[h_idx] = delta;
        state.head = (state.head + 1) % 16;

        let d_v = state.current_depth as u128;
        let mut sum: u128 = 0;
        let c_h = state.head as usize;
        for i in 0..state.current_depth as usize {
            sum += state.history[(c_h + 16 - 1 - i) % 16] as u128;
        }
        let avg = sum / d_v;
        let mut v_sum: u128 = 0;
        for i in 0..state.current_depth as usize {
            let val = state.history[(c_h + 16 - 1 - i) % 16] as u128;
            let diff = if val > avg { val - avg } else { avg - val };
            v_sum = v_sum.saturating_add(diff.saturating_mul(diff));
        }

        let m_v = v_sum / d_v;
        let msb = 128 - m_v.leading_zeros() as i32;
        let f_log = if msb > 8 { ((msb as u64) << 8) + ((m_v >> (msb - 8)) & 0xFF) as u64 } 
                    else if msb > 0 { (msb as u64) << 8 } else { 0 };
        
        state.prev_variance_index = state.variance_index;
        state.variance_index = (state.variance_index.saturating_mul(4).saturating_add(f_log)) / 5;

        // Реляционная логика управления (x_control)
        let x_max = state.variance_index.max(state.epsilon);
        let x_step = x_max / 10;
        let grad = state.variance_index as i64 - state.prev_variance_index as i64;
        let thr = state.variance_index.saturating_mul(state.gradient_threshold_percent) / 100;
        let phi_crit = state.epsilon.saturating_mul(2);

        match state.mode {
            0 => { // S0: Покой
                if state.variance_index > phi_crit {
                    state.mode = 2; state.x_control = x_step;
                }
            },
            2 => { // S2: Эволюция (Набор давления)
                if state.variance_index < state.epsilon && grad.abs() < (thr as i64) {
                    state.mode = 0; state.x_control = 0;
                } else {
                    state.x_control = state.x_control.saturating_add(x_step).min(x_max);
                    if state.x_control >= (x_max * 9 / 10) { state.mode = 1; }
                }
            },
            1 => { // S1: Контроль (Максимальное давление)
                state.x_control = x_max;
                if state.variance_index <= phi_crit && grad <= 0 {
                    state.mode = 0; state.x_control = 0;
                } else if grad > (thr as i64 * 3) {
                    state.mode = 2; state.x_control = x_max / 2;
                }
            },
            _ => state.mode = 2,
        }

        // Резонанс (Минтинг в S0)
        if state.mode == 0 && delta < state.epsilon {
            let seeds = &[b"orbital-genesis".as_ref(), &[ctx.bumps.mint]];
            token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), MintTo {
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.token_account.to_account_info(),
                authority: ctx.accounts.mint.to_account_info(),
            }, &[&seeds[..]]), 100_000_000)?;
        }

        state.last_noise = noise;
        Ok(())
    }
}

#[account]
pub struct PulseState {
    pub authority: Pubkey, pub last_noise: u64, pub epsilon: u64,
    pub history: [u64; 16], pub current_depth: u8, pub head: u8,                 
    pub variance_index: u64, pub prev_variance_index: u64,
    pub gradient_threshold_percent: u64, pub x_control: u64, 
    pub mode: u8, pub calib_count: u8, pub is_born: bool,
}

impl PulseState { pub const LEN: usize = 8 + 32 + 8 + 8 + 128 + 1 + 1 + 8 + 8 + 8 + 8 + 1 + 1 + 1; }

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = signer, space = PulseState::LEN)] pub state: Account<'info, PulseState>,
    #[account(init_if_needed, payer = signer, mint::decimals = 9, mint::authority = mint, seeds = [b"orbital-genesis"], bump)] pub mint: Account<'info, Mint>,
    #[account(init_if_needed, payer = signer, associated_token::mint = mint, associated_token::authority = signer)] pub token_account: Account<'info, TokenAccount>,
    #[account(mut)] pub signer: Signer<'info>, pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>, pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Transition<'info> {
    #[account(mut)] pub state: Account<'info, PulseState>,
    #[account(mut, seeds = [b"orbital-genesis"], bump)] pub mint: Account<'info, Mint>,
    #[account(mut)] pub token_account: Account<'info, TokenAccount>,
    #[account(address = slot_hashes::ID)] pub slot_hashes: AccountInfo<'info>,
    #[account(mut)] pub signer: Signer<'info>, pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum ErrorCode { #[msg("Hash Not Found")] HashNotFound, #[msg("Invalid Depth")] InvalidDepth }
