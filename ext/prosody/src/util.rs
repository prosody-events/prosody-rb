#[macro_export]
macro_rules! id {
    ($str:expr) => {{
        static VAL: magnus::value::LazyId = magnus::value::LazyId::new($str);
        *VAL
    }};
}
