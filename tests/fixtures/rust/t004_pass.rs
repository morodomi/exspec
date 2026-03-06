use rstest::rstest;

#[rstest]
#[case(1, 2, 3)]
#[case(4, 5, 9)]
fn test_add(#[case] a: i32, #[case] b: i32, #[case] expected: i32) {
    assert_eq!(a + b, expected);
}
