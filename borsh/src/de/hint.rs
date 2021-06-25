#[inline]
pub(crate) fn cautious<T>(hint: usize) -> usize {
    let el_size = core::mem::size_of::<T>();
    core::cmp::max(core::cmp::min(hint, 4096 / el_size), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_cautious_u8() {
        assert_eq!(cautious::<u8>(10), 10);
    }

    #[test]
    pub fn test_cautious_zero() {
        assert_eq!(cautious::<u8>(0), 1);
    }
}
