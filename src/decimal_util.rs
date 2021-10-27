use rust_decimal::Decimal;

pub trait AsDecimal {
    fn as_decimal(&self) -> Decimal;
}

impl AsDecimal for u64 {
    fn as_decimal(&self) -> Decimal {
        Decimal::from(*self)
        // Decimal::from_i128_with_scale(*self as i128, 0)
    }
}

pub trait DecimalExt {
    fn format_percentage(&self) -> String;
}

impl DecimalExt for Decimal {
    fn format_percentage(&self) -> String {
        format!("{} %", (self * Decimal::from(100)).round_dp(2).normalize())
    }
}
