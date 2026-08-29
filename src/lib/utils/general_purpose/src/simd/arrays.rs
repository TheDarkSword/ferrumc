#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
use std::arch::x86_64::*;

#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn has_avx2() -> bool {
    is_x86_feature_detected!("avx2")
}

/// Converts a slice of `u8` to a slice of `i8` without copying.
pub const fn u8_slice_to_i8(input: &[u8]) -> &[i8] {
    // SAFETY: u8 and i8 have the same size, alignment and valid bit-patterns
    unsafe { std::mem::transmute(input) }
}

/// Converts a slice of `u8` to a `Vec<u32>` in big-endian order.
pub fn u8_slice_to_u32_be(input: &[u8]) -> Vec<u32> {
    assert_eq!(
        input.len() % 4,
        0,
        "Input length must be a multiple of 4 for u32 conversion"
    );

    #[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
    if has_avx2() {
        return unsafe { u8_slice_to_u32_be_simd(input) };
    }
    u8_slice_to_u32_be_normal(input)
}

fn u8_slice_to_u32_be_normal(input: &[u8]) -> Vec<u32> {
    input
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_be_bytes(*chunk))
        .collect()
}

#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
#[target_feature(enable = "avx2")]
unsafe fn u8_slice_to_u32_be_simd(input: &[u8]) -> Vec<u32> {
    debug_assert_eq!(
        input.len() % 4,
        0,
        "Input length must be a multiple of 4 for u32 conversion"
    );

    let mut output: Vec<u32> = Vec::new();
    output.reserve_exact(input.len() / 4);

    let shuffle_mask = _mm256_setr_epi8(
        3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12, 19, 18, 17, 16, 23, 22, 21, 20, 27,
        26, 25, 24, 31, 30, 29, 28,
    );
    let (chunks, rest) = input.as_chunks::<32>();
    for (i, chunk) in chunks.iter().enumerate() {
        let out = output.as_mut_ptr().cast::<__m256i>().add(i);
        let data = _mm256_loadu_si256(chunk.as_ptr().cast());
        let shuffled = _mm256_shuffle_epi8(data, shuffle_mask);
        _mm256_storeu_si256(out, shuffled);
        output.set_len((i + 1) * 8);
    }

    for chunk in rest.as_chunks::<4>().0 {
        output.push(u32::from_be_bytes(*chunk));
    }

    output
}

pub fn u8_slice_to_i32_be(input: &[u8]) -> Vec<i32> {
    let out = u8_slice_to_u32_be(input);
    // SAFETY: u32 and i32 have the same size, alignment and valid bit-patterns
    unsafe { std::mem::transmute(out) }
}

pub fn u8_slice_to_u64_be(input: &[u8]) -> Vec<u64> {
    assert_eq!(
        input.len() % 8,
        0,
        "Input length must be a multiple of 8 for u64 conversion"
    );

    #[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
    if has_avx2() {
        return unsafe { u8_slice_to_u64_be_simd(input) };
    }
    u8_slice_to_u64_be_normal(input)
}

fn u8_slice_to_u64_be_normal(input: &[u8]) -> Vec<u64> {
    input
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_be_bytes(*chunk))
        .collect()
}

#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
#[target_feature(enable = "avx2")]
unsafe fn u8_slice_to_u64_be_simd(input: &[u8]) -> Vec<u64> {
    debug_assert_eq!(
        input.len() % 8,
        0,
        "Input length must be a multiple of 8 for u64 conversion"
    );

    let mut output: Vec<u64> = Vec::new();
    output.reserve_exact(input.len() / 8);

    let shuffle_mask = _mm256_setr_epi8(
        7, 6, 5, 4, 3, 2, 1, 0, // Reverse first u64
        15, 14, 13, 12, 11, 10, 9, 8, // Reverse second u64
        23, 22, 21, 20, 19, 18, 17, 16, // Reverse third u64
        31, 30, 29, 28, 27, 26, 25, 24, // Reverse fourth u64
    );

    let (chunks, rest) = input.as_chunks::<32>();
    for (i, chunk) in chunks.iter().enumerate() {
        let out = output.as_mut_ptr().cast::<__m256i>().add(i);
        let data = _mm256_loadu_si256(chunk.as_ptr().cast());
        let shuffled = _mm256_shuffle_epi8(data, shuffle_mask);
        _mm256_storeu_si256(out, shuffled);
        output.set_len((i + 1) * 4);
    }

    for chunk in rest.as_chunks::<8>().0 {
        output.push(u64::from_be_bytes(*chunk));
    }

    output
}

pub fn u8_slice_to_i64_be(input: &[u8]) -> Vec<i64> {
    let out = u8_slice_to_u64_be(input);
    // SAFETY: u64 and i64 have the same size, alignment and valid bit-patterns
    unsafe { std::mem::transmute(out) }
}

pub fn u32_slice_to_u8_be(input: &[u32]) -> Vec<u8> {
    #[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
    if has_avx2() {
        return unsafe { u32_slice_to_u8_be_simd(input) };
    }
    u32_slice_to_u8_be_normal(input)
}

fn u32_slice_to_u8_be_normal(input: &[u32]) -> Vec<u8> {
    input.iter().flat_map(|val| val.to_be_bytes()).collect()
}

#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
#[target_feature(enable = "avx2")]
unsafe fn u32_slice_to_u8_be_simd(input: &[u32]) -> Vec<u8> {
    let mut output: Vec<u8> = Vec::new();
    output.reserve_exact(input.len() * 4);

    let shuffle_mask = _mm256_setr_epi8(
        3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12, 19, 18, 17, 16, 23, 22, 21, 20, 27,
        26, 25, 24, 31, 30, 29, 28,
    );

    let (chunks, rest) = input.as_chunks::<8>();
    for (i, chunk) in chunks.iter().enumerate() {
        let out = output.as_mut_ptr().cast::<__m256i>().add(i);
        let data = _mm256_loadu_si256(chunk.as_ptr().cast());
        let shuffled = _mm256_shuffle_epi8(data, shuffle_mask);
        _mm256_storeu_si256(out, shuffled);
        output.set_len((i + 1) * 32);
    }

    for val in rest {
        let val = val.to_be_bytes();
        output.extend_from_slice(&val);
    }

    output
}

pub fn u64_slice_to_u8_be(input: &[u64]) -> Vec<u8> {
    #[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
    if has_avx2() {
        return unsafe { u64_slice_to_u8_be_simd(input) };
    }
    u64_slice_to_u8_be_normal(input)
}

fn u64_slice_to_u8_be_normal(input: &[u64]) -> Vec<u8> {
    input.iter().flat_map(|val| val.to_be_bytes()).collect()
}

#[cfg(all(target_arch = "x86_64", not(target_os = "macos")))]
#[target_feature(enable = "avx2")]
unsafe fn u64_slice_to_u8_be_simd(input: &[u64]) -> Vec<u8> {
    let mut output: Vec<u8> = Vec::new();
    output.reserve_exact(input.len() * 8);

    let shuffle_mask = _mm256_setr_epi8(
        7, 6, 5, 4, 3, 2, 1, 0, // Reverse first u64
        15, 14, 13, 12, 11, 10, 9, 8, // Reverse second u64
        23, 22, 21, 20, 19, 18, 17, 16, // Reverse third u64
        31, 30, 29, 28, 27, 26, 25, 24, // Reverse fourth u64
    );

    let (chunks, rest) = input.as_chunks::<4>();
    for (i, chunk) in chunks.iter().enumerate() {
        let out = output.as_mut_ptr().cast::<__m256i>().add(i);
        let data = _mm256_loadu_si256(chunk.as_ptr().cast());
        let shuffled = _mm256_shuffle_epi8(data, shuffle_mask);
        _mm256_storeu_si256(out, shuffled);
        output.set_len((i + 1) * 32);
    }

    for val in rest {
        let val = val.to_be_bytes();
        output.extend_from_slice(&val);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lengths that leave a non-empty remainder after the 32-byte SIMD stride,
    /// which is where the scalar tail runs.
    #[test]
    fn u32_be_matches_scalar_across_tail_lengths() {
        for words in 0..24usize {
            let input: Vec<u8> = (0..words * 4).map(|i| i as u8).collect();
            let expected: Vec<u32> = input
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_be_bytes(*c))
                .collect();
            assert_eq!(u8_slice_to_u32_be(&input), expected, "words = {words}");
        }
    }

    #[test]
    fn u64_be_matches_scalar_across_tail_lengths() {
        for words in 0..24usize {
            let input: Vec<u8> = (0..words * 8).map(|i| i as u8).collect();
            let expected: Vec<u64> = input
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| u64::from_be_bytes(*c))
                .collect();
            assert_eq!(u8_slice_to_u64_be(&input), expected, "words = {words}");
        }
    }

    #[test]
    fn round_trips_through_byte_form() {
        let words: Vec<u32> = (0..37u32).map(|i| i.wrapping_mul(0x0101_1001)).collect();
        assert_eq!(u8_slice_to_u32_be(&u32_slice_to_u8_be(&words)), words);

        let words: Vec<u64> = (0..37u64)
            .map(|i| i.wrapping_mul(0x0101_1001_0110_0011))
            .collect();
        assert_eq!(u8_slice_to_u64_be(&u64_slice_to_u8_be(&words)), words);
    }
}
