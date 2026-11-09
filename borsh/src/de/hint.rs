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

    /// Sizes that used to truncate to 0 and 1 as `u32`.
    #[cfg(target_pointer_width = "64")]
    #[test]
    pub fn test_cautious_huge_elements() {
        assert_eq!(cautious::<[u8; 1 << 32]>(u32::MAX), 1);
        assert_eq!(cautious::<[u8; (1 << 32) + 1]>(u32::MAX), 1);
    }
}
