//! A secret's fingerprint: what the journal records in place of a secret, and what a
//! contract answers in place of a credential it was applied.

/// How many hex characters a fingerprint is written with.
const WIDTH: usize = 4;

/// A short, stable likeness of a value, for saying that two copies of a credential
/// disagree without printing either of them.
///
/// Sixteen bits, which is enough to notice a difference and far too few to work back
/// to the value: a fingerprint names about one in sixty-five thousand possible
/// values, so it identifies nothing on its own. It is a label, not a proof, and the
/// narrowness is the point — a longer one would start to be worth attacking.
///
/// Stable across runs and across versions, which a hasher from the standard library
/// is not: `DefaultHasher`'s output is explicitly allowed to change between
/// releases, and a fingerprint that moved under an upgrade would report every
/// credential in the stack as newly regenerated.
#[must_use]
pub fn fingerprint(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("~{:0WIDTH$x}", hash & 0xffff)
}

#[cfg(test)]
mod tests;
