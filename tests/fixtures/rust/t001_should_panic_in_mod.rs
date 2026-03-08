#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_divide_by_zero_in_mod() {
        divide(1, 0);
    }
}
