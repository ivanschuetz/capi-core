use rust_decimal::Decimal;

pub trait AsDecimal {
    fn as_decimal(&self) -> Decimal;
}

impl AsDecimal for u64 {
    fn as_decimal(&self) -> Decimal {
        Decimal::from(*self)
    }
}

pub trait DecimalExt {
    fn format_percentage(&self) -> String;

    /// How much do I have to pay to get self after deducting the fee?
    /// This could be more abstract, but unlikely to be used for anything else and it's easier to understand like this.
    fn amount_to_pay_to_get_self_after_deducting_fee(&self, fee_perc: Decimal) -> Decimal;
}

impl DecimalExt for Decimal {
    fn format_percentage(&self) -> String {
        format!("{} %", (self * Decimal::from(100)).round_dp(2).normalize())
    }

    fn amount_to_pay_to_get_self_after_deducting_fee(&self, fee_perc: Decimal) -> Decimal {
        self / (1_u64.as_decimal() - fee_perc)
    }
}
