use std::time::{SystemTime, UNIX_EPOCH, Duration};
use rand::{Rng, OsRng};

static ALPH: [char; 64] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-', '_'
];

/// Generate simple uid string.
pub fn uid() -> String {
    // Get time part
    let unix_time = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur,
        Err(_) => Duration::new(0, 0),
    };
    let mut ns: u32 = unix_time.subsec_nanos();

    // Get rand part
    let mut rd: u64 = match OsRng::new() {
        Ok(mut rng) => rng.gen(),
        Err(_) => 0,
    };

    // Result string
    let mut output = String::with_capacity(12);

    // Rand part
    for _ in 0..7 {
        output.push(ALPH[(rd & 63) as usize]);
        rd >>= 6;
    }

    // Time part
    for _ in 0..5 {
        output.push(ALPH[(ns & 63) as usize]);
        ns >>= 6;
    }

    return output;
}

/// Generate numerical id.
pub fn nid() -> u64 {
    // Get time part
    let unix_time = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur,
        Err(_) => Duration::new(0, 0),
    };
    let ns: u32 = unix_time.subsec_nanos();

    // Get rand part
    let rd: u64 = match OsRng::new() {
        Ok(mut rng) => rng.gen(),
        Err(_) => 0,
    };

    // Result
    ((rd as u64) << 32) | ns as u64
}

/// Generate 12-bytes id.
pub fn bid() -> [u8; 12] {
    let mut out: [u8; 12] = [0; 12];

    // Get time part
    let unix_time = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur,
        Err(_) => Duration::new(0, 0),
    };
    let ns: u32 = unix_time.subsec_nanos();
    // todo: wait for stabilization to_bytes()
    out[0] = ((ns & 0xff_00_00_00) >> 24) as u8;
    out[1] = ((ns & 0x00_ff_00_00) >> 16) as u8;
    out[2] = ((ns & 0x00_00_ff_00) >> 8) as u8;
    out[3] = (ns & 0x00_00_00_ff) as u8;

    // Get rand part
    let rd: [u8; 10] = match OsRng::new() {
        Ok(mut rng) => rng.gen(),
        Err(_) => [0; 10],
    };

    for i in 4..12 {
        out[i] = rd[i - 4];
    }

    return out
}

/// Convert 12-bytes id to u128.
pub fn bid_to_u128(bid: &[u8]) -> u128 {
    // todo: wait for stabilization to_bytes()
    let mut id: u128 = 0;
    id += (bid[0] as u128) << 88;
    id += (bid[1] as u128) << 80;
    id += (bid[2] as u128) << 72;
    id += (bid[3] as u128) << 64;
    id += (bid[4] as u128) << 56;
    id += (bid[5] as u128) << 48;
    id += (bid[6] as u128) << 40;
    id += (bid[7] as u128) << 32;
    id += (bid[8] as u128) << 24;
    id += (bid[9] as u128) << 16;
    id += (bid[10] as u128) << 8;
    id += bid[11] as u128;
    return id
}

/// Convert u128 to 12-bytes id.
pub fn u128_to_bytes(num: u128) -> [u8; 12] {
    let mut out: [u8; 12] = [0; 12];
    out[0] =  ((num & 0xff_00_00_00_00_00_00_00_00_00_00_00) >> 88) as u8;
    out[1] =  ((num & 0x00_ff_00_00_00_00_00_00_00_00_00_00) >> 80) as u8;
    out[2] =  ((num & 0x00_00_ff_00_00_00_00_00_00_00_00_00) >> 72) as u8;
    out[3] =  ((num & 0x00_00_00_ff_00_00_00_00_00_00_00_00) >> 64) as u8;
    out[4] =  ((num & 0x00_00_00_00_ff_00_00_00_00_00_00_00) >> 56) as u8;
    out[5] =  ((num & 0x00_00_00_00_00_ff_00_00_00_00_00_00) >> 48) as u8;
    out[6] = ((num & 0x00_00_00_00_00_00_ff_00_00_00_00_00) >> 40) as u8;
    out[7] = ((num & 0x00_00_00_00_00_00_00_ff_00_00_00_00) >> 32) as u8;
    out[8] = ((num & 0x00_00_00_00_00_00_00_00_ff_00_00_00) >> 24) as u8;
    out[9] = ((num & 0x00_00_00_00_00_00_00_00_00_ff_00_00) >> 16) as u8;
    out[10] = ((num & 0x00_00_00_00_00_00_00_00_00_00_ff_00) >> 8) as u8;
    out[11] =  (num & 0x00_00_00_00_00_00_00_00_00_00_00_ff) as u8;
    return out
}

/// Convert u64 to bytes (wait stabilization of u64.to_bytes()).
pub fn u64_to_bytes(num: u64) -> [u8; 8] {
    let mut out: [u8; 8] = [0; 8];
    out[0] = ((num & 0xff_00_00_00_00_00_00_00) >> 56) as u8;
    out[1] = ((num & 0x00_ff_00_00_00_00_00_00) >> 48) as u8;
    out[2] = ((num & 0x00_00_ff_00_00_00_00_00) >> 40) as u8;
    out[3] = ((num & 0x00_00_00_ff_00_00_00_00) >> 32) as u8;
    out[4] = ((num & 0x00_00_00_00_ff_00_00_00) >> 24) as u8;
    out[5] = ((num & 0x00_00_00_00_00_ff_00_00) >> 16) as u8;
    out[6] = ((num & 0x00_00_00_00_00_00_ff_00) >> 8) as u8;
    out[7] = (num & 0x00_00_00_00_00_00_00_ff) as u8;
    return out
}

/// Convert 8bytes to u64 (wait stabilization of u64.to_bytes()).
pub fn bytes_to_u64(bin: &[u8]) -> u64 {
    let mut out: u64 = 0;
    out += (bin[0] as u64) << 56;
    out += (bin[1] as u64) << 48;
    out += (bin[2] as u64) << 40;
    out += (bin[3] as u64) << 32;
    out += (bin[4] as u64) << 24;
    out += (bin[5] as u64) << 16;
    out += (bin[6] as u64) << 8;
    out += bin[7] as u64;
    return out
}

// -----------------------------
// --- --- --- Tests --- --- ---
// -----------------------------
#[cfg(test)]
mod tests {
    use utils::*;

    #[test]
    fn uid_dif() {
        assert!(uid() != "aaaaaaaaaaaa" && uid() != uid());
    }

    #[test]
    fn nid_dif() {
        assert!(nid() != 0 && nid() != nid());
    }

    #[test]
    fn bid_dif() {
        assert!(bid() != [0u8; 12] && bid() != bid());
    }
}
