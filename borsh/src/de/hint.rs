#[inline]
pub fn cautious<T>(hint: u32) -> usize {
    let el_size = core::mem::size_of::<T>();
    core::cmp::max(core::cmp::min(hint as usize, 4096 / el_size), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_cautious_u8() {
        assert_eq!(cautious::<u8>(10), 10);
    }

    #[test]
    pub fn test_cautious_caps_at_4096_bytes() {
        assert_eq!(cautious::<u64>(u32::MAX), 512);
    }

    #[test]
    pub fn test_cautious_element_over_cap() {
        // `4096 / 4097 == 0`, so the floor of one element decides.
        assert_eq!(cautious::<[u8; 4097]>(u32::MAX), 1);
        assert_eq!(cautious::<[u8; 4097]>(0), 1);
    }

    /// A size that used to truncate to 0 as `u32` (division by zero).
    #[cfg(target_pointer_width = "64")]
    #[test]
    pub fn test_cautious_huge_element_multiple_of_2_32() {
        assert_eq!(cautious::<[u8; 1 << 32]>(u32::MAX), 1);
    }

    /// A size that used to truncate to 1 as `u32` (4096 elements of 4 GiB).
    #[cfg(target_pointer_width = "64")]
    #[test]
    pub fn test_cautious_huge_element_truncated_to_one() {
        assert_eq!(cautious::<[u8; (1 << 32) + 1]>(u32::MAX), 1);
    }
}
