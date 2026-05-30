/// 无符号整数的环形比较（判断 lhs 是否小于 rhs，考虑溢出）
/// 因为序列号是 u16/u32，会发生回绕（如 65535 -> 0），
/// 不能直接用 `<` 比较，必须利用补码特性在环形空间内比较大小。
#[inline]
pub fn wrapping_less(lhs: u32, rhs: u32) -> bool {
    rhs.wrapping_sub(lhs) as i32 > 0   
}

#[inline]
pub fn wrapping_less_u16(lhs: u16, rhs: u16) -> bool {
    rhs.wrapping_sub(lhs) as i16 > 0   
}

/// 环形最小值比较（考虑溢出）
#[inline]
pub fn wrapping_min(a: u32, b: u32) -> u32 {
    if wrapping_less(a, b) { a } else { b }
}