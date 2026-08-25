pub const FP_DECIMALS: u32 = 12;
pub const FP_ONE: u128 = 1_000_000_000_000;

// e^29 ≈ 3.9e12, so for x ≤ -29 the fixed-point reciprocal
// (FP_ONE*FP_ONE / exp(-x)) truncates to 0. Clamp early to avoid
// running the Taylor series for large |x|, which would overflow u128.
// `29 * FP_ONE` is x in 12.12 fixed point (29.0 × 1e12 = 2.9e13).
const EXP_NEG_UNDERFLOW: i128 = -(29 * FP_ONE as i128);

pub fn exp(x: i128) -> Option<u128> {
    if x == 0 {
        return Some(FP_ONE);
    }
    if x <= EXP_NEG_UNDERFLOW {
        return Some(0);
    }
    if x < 0 {
        let pos = exp(-x)?;
        if pos == 0 {
            return Some(0);
        }
        return Some(FP_ONE.checked_mul(FP_ONE)?.checked_div(pos)?);
    }
    let x_u = x as u128;
    let mut result = FP_ONE;
    let mut term = FP_ONE;
    for n in 1..32 {
        term = term.checked_mul(x_u)?.checked_div(FP_ONE)?.checked_div(n as u128)?;
        result = result.checked_add(term)?;
        if term == 0 {
            break;
        }
    }
    Some(result)
}

pub fn ln(x: u128) -> Option<i128> {
    if x == 0 {
        return None;
    }
    if x < FP_ONE {
        let inv = FP_ONE.checked_mul(FP_ONE)?.checked_div(x)?;
        let result = ln(inv)?;
        return Some(-result);
    }
    let one = FP_ONE as i128;
    let mut scaled = x;
    let mut log2_factor = 0i128;
    while scaled >= 2 * FP_ONE {
        scaled = scaled / 2;
        log2_factor += FP_ONE as i128;
    }
    let ln2 = 693147180559i128; // ln(2) × FP_ONE, rounded (ln(2) ≈ 0.69314718)
    let y_num = (scaled as i128).checked_sub(one)?;
    let y = (y_num).checked_mul(one)?.checked_div(scaled as i128 + one)?;
    let y_sq = ((y as u128).checked_mul(y as u128)?).checked_div(FP_ONE)?;
    let mut term = y;
    let mut result = y;
    for n in (3..64).step_by(2) {
        term = ((term as u128).checked_mul(y_sq)?.checked_div(FP_ONE)?) as i128;
        let term_div = term.checked_div(n as i128)?;
        result = result.checked_add(term_div)?;
        if term == 0 {
            break;
        }
    }
    result = result.checked_add(result)?;
    result = result.checked_add(log2_factor.checked_mul(ln2)?.checked_div(FP_ONE as i128)?)?;
    Some(result)
}

pub fn lmsr_cost(q_yes: u64, q_no: u64, b: u64) -> Option<u64> {
    let max_q = std::cmp::max(q_yes, q_no);
    let b_u = b as u128;
    let max_q_u = max_q as u128;

    let z_yes = if q_yes >= max_q {
        0i128
    } else {
        let diff = (max_q - q_yes) as u128;
        let val = diff.checked_mul(FP_ONE)?.checked_div(b_u)?;
        -(val as i128)
    };
    let z_no = if q_no >= max_q {
        0i128
    } else {
        let diff = (max_q - q_no) as u128;
        let val = diff.checked_mul(FP_ONE)?.checked_div(b_u)?;
        -(val as i128)
    };
    let e_yes = exp(z_yes)?;
    let e_no = exp(z_no)?;
    let sum = e_yes.checked_add(e_no)?;
    let ln_sum = ln(sum)?;
    let b_ln = b_u.checked_mul(ln_sum as u128)?.checked_div(FP_ONE)?;
    let cost = max_q_u.checked_add(b_ln)?;
    u64::try_from(cost).ok()
}

pub fn lmsr_buy_cost(
    q_yes: u64,
    q_no: u64,
    outcome: u8,
    shares: u64,
    b: u64,
) -> Option<u64> {
    let cost_before = lmsr_cost(q_yes, q_no, b)?;
    let (new_q_yes, new_q_no) = if outcome == 0 {
        (q_yes.checked_add(shares)?, q_no)
    } else {
        (q_yes, q_no.checked_add(shares)?)
    };
    let cost_after = lmsr_cost(new_q_yes, new_q_no, b)?;
    cost_after.checked_sub(cost_before)
}