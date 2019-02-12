#[macro_export]
macro_rules! choose {
    ($cond: expr => $true_val: expr, $false_val: expr) => {{
        if $cond {
            $true_val
        } else {
            $false_val
        }
    }};
}
