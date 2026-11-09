//! Harnesses over the keleusma-wire load-time trust path, the majority-of-three
//! vote, the cyclic redundancy check (CRC) and the single-error-correct,
//! double-error-detect (SECDED) word decoder.

use keleusma_wire::ecc::{check_byte, decode_word};
use keleusma_wire::scalar::maj3;
use keleusma_wire::{WordStatus, crc32};

/// The majority vote is the bitwise majority and outvotes any single copy.
///
/// Property. For all bytes `a`, `b` and `c`, each bit of `maj3(a, b, c)` is
/// set exactly when at least two of the three input bits are set, the vote is
/// symmetric in its arguments, and two agreeing copies win against any third.
#[kani::proof]
fn maj3_is_bitwise_majority() {
    let a: u8 = kani::any();
    let b: u8 = kani::any();
    let c: u8 = kani::any();
    let m = maj3(a, b, c);
    let mut bit = 0;
    while bit < 8 {
        let votes = ((a >> bit) & 1) + ((b >> bit) & 1) + ((c >> bit) & 1);
        assert_eq!((m >> bit) & 1 == 1, votes >= 2);
        bit += 1;
    }
    assert_eq!(m, maj3(b, c, a));
    assert_eq!(m, maj3(b, a, c));
    assert_eq!(maj3(a, a, c), a);
    assert_eq!(maj3(a, c, a), a);
    assert_eq!(maj3(c, a, a), a);
}

/// The cyclic redundancy check detects every single-bit and every double-bit
/// error in a short message.
///
/// Property. For every message of exactly `LEN` bytes and every choice of one
/// or two distinct bit positions, flipping those bits changes `crc32`.
/// `crc32` does not panic on any input of that length.
fn crc_detects_one_and_two_bit_errors<const LEN: usize>() {
    let msg: [u8; LEN] = kani::any();
    let i: usize = kani::any();
    let j: usize = kani::any();
    kani::assume(i < LEN * 8 && j < LEN * 8 && i != j);
    let mut one = msg;
    one[i / 8] ^= 1 << (i % 8);
    let mut two = one;
    two[j / 8] ^= 1 << (j % 8);
    let clean = crc32(&msg);
    assert_ne!(clean, crc32(&one));
    assert_ne!(clean, crc32(&two));
}

#[kani::proof]
#[kani::unwind(9)]
fn crc32_detects_bit_errors_in_four_bytes() {
    crc_detects_one_and_two_bit_errors::<4>();
}

/// Flips bit `i` of the 72-bit codeword formed by `data` and `check`, with the
/// 64 data bits first and the 8 check bits after them.
fn flip(data: u64, check: u8, i: u32) -> (u64, u8) {
    if i < 64 {
        (data ^ (1u64 << i), check)
    } else {
        (data, check ^ (1u8 << (i - 64)))
    }
}

/// The SECDED decoder accepts a clean word and corrects any single-bit error.
///
/// Property. For every 64-bit word, decoding it against its own check byte
/// reports it clean, and flipping any one of the 72 codeword bits decodes as
/// corrected to the original word.
#[kani::proof]
#[kani::unwind(73)]
fn secded_corrects_single_bit_errors() {
    let data: u64 = kani::any();
    let check = check_byte(data);
    assert_eq!(decode_word(data, check), WordStatus::Clean(data));
    let i: u32 = kani::any();
    kani::assume(i < 72);
    let (d1, c1) = flip(data, check, i);
    assert_eq!(decode_word(d1, c1), WordStatus::Corrected(data));
}

/// The SECDED decoder never miscorrects a double-bit error.
///
/// Property. For every 64-bit word and every pair of distinct codeword bit
/// positions, flipping both bits decodes as uncorrectable.
#[kani::proof]
#[kani::unwind(73)]
fn secded_detects_double_bit_errors() {
    let data: u64 = kani::any();
    let check = check_byte(data);
    let i: u32 = kani::any();
    let j: u32 = kani::any();
    kani::assume(i < 72 && j < 72 && i != j);
    let (d1, c1) = flip(data, check, i);
    let (d2, c2) = flip(d1, c1, j);
    assert_eq!(decode_word(d2, c2), WordStatus::Uncorrectable);
}
