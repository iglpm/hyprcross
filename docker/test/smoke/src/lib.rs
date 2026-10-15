//! Rust calling C, on every target hyprcross has an image for (bare metal included: no_std).

#![no_std]

unsafe extern "C" {
    fn hyprcross_answer() -> i32;
}

pub fn answer() -> i32 {
    // SAFETY: a C function without arguments or state.
    unsafe { hyprcross_answer() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn c_code_is_compiled_linked_and_run() {
        assert_eq!(super::answer(), 42);
    }
}
