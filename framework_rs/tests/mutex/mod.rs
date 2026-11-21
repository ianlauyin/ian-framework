use std::sync::Mutex;

use ian_framework_rs::mutex;

#[test]
fn a_poisoned_mutex_returns_the_value_and_clears_the_poison() {
    let mutex = Mutex::new(7);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = mutex.lock().unwrap();
        panic!("poison");
    }));
    assert!(mutex.is_poisoned());
    {
        let guard = mutex::lock(&mutex);
        assert_eq!(*guard, 7);
    }
    assert!(!mutex.is_poisoned());
}
