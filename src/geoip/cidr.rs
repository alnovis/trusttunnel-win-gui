//! CIDR helpers shared by the RIR parser.

/// Convert an address count (a power of two, as RIR delegated files report) to
/// a prefix length. Mirrors the Keenetic S97geoip `count_to_cidr`:
///   count=256 -> /24, count=1 -> /32.
///
/// Non-power-of-two counts are rounded down to the enclosing prefix (same as
/// the shell loop, which halves until <= 1).
pub fn count_to_prefix(mut count: u64) -> u8 {
    let mut prefix: u8 = 32;
    while count > 1 {
        count /= 2;
        prefix = prefix.saturating_sub(1);
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_counts() {
        assert_eq!(count_to_prefix(1), 32);
        assert_eq!(count_to_prefix(256), 24);
        assert_eq!(count_to_prefix(65536), 16);
        assert_eq!(count_to_prefix(1024), 22);
    }
}
