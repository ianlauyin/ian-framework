use google_cloud_storage::model::Object;

use crate::gcp::storage::object::*;

#[test]
fn an_object_gives_its_meta() {
    let object = Object::new()
        .set_name("a.txt")
        .set_size(7)
        .set_content_type("text/plain")
        .set_metadata([("k", "v")]);
    let meta = ObjectMeta::from(object);
    assert_eq!(meta.name, "a.txt");
    assert_eq!(meta.size, 7);
    assert_eq!(meta.content_type.as_deref(), Some("text/plain"));
    assert_eq!(meta.metadata.get("k").map(String::as_str), Some("v"));
    assert_eq!(meta.updated, None);
}

#[test]
fn an_empty_content_type_and_a_negative_size_are_not_kept() {
    let meta = ObjectMeta::from(Object::new().set_name("a").set_size(-1));
    assert_eq!(meta.size, 0);
    assert_eq!(meta.content_type, None);
}
