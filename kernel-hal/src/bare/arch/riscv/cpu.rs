hal_fn_impl! {
    impl mod crate::defs::cpu {
        fn cpu_frequency() -> u16 {
            const DEFAULT: u16 = 2600;
            DEFAULT
        }
    }
}
