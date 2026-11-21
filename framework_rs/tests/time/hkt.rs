use ian_framework_rs::time::hkt_now;

#[test]
fn hkt_now_uses_the_hong_kong_offset() {
    assert_eq!(hkt_now().offset().local_minus_utc(), 8 * 3600);
}
