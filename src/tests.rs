use crate::{
    Dyn, DynTypeId,
    traitcast::{AsTraitObject, cast_trait_object, match_dyn_type_id},
};

// This library relies mainly on doctests for testing, but the tests in here
// are intended to cover some specific intended use-cases to help make sure they
// continue to be possible under future changes.

struct Implementer;
struct NotImplementer;

trait WithMessage {
    fn message(&self) -> &'static str;
}

impl AsTraitObject for Implementer {
    fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<Dyn<'a>> {
        match_dyn_type_id!(self, type_id => WithMessage)
    }
}

impl AsTraitObject for NotImplementer {
    // Intentionally uses default implementation, which reports that no
    // trait object types are supported for casting.
}

impl WithMessage for Implementer {
    fn message(&self) -> &'static str {
        "hello from implementer"
    }
}

#[test]
fn boxed() {
    extern crate alloc;
    use alloc::boxed::Box;

    let concrete_boxed = Box::new(Implementer);
    let trait_object_boxed = concrete_boxed as Box<dyn AsTraitObject>;
    let with_message_ref = cast_trait_object::<dyn WithMessage>(&*trait_object_boxed)
        .expect("can't cast to WithMessage");
    assert_eq!(with_message_ref.message(), "hello from implementer");
}

#[test]
fn arced() {
    extern crate alloc;
    use alloc::sync::Arc;

    let concrete_arc = Arc::new(Implementer);
    let other_owner = Arc::clone(&concrete_arc);
    let trait_object_arc = concrete_arc as Arc<dyn AsTraitObject>;
    let with_message_ref = cast_trait_object::<dyn WithMessage>(&*trait_object_arc)
        .expect("can't cast to WithMessage");
    assert_eq!(with_message_ref.message(), "hello from implementer");
    assert_eq!(other_owner.message(), "hello from implementer");
}

#[test]
fn dyn_handles() {
    extern crate alloc;
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    // This test covers the main case this library was written to help support:
    // a table of differently-typed objects that each implement different
    // combinations of traits, which callers from outside the process can
    // attempt to use in different ways and have it succeed or fail depending
    // on which trait casts are successful at runtime.
    //
    // A practical implementation of that pattern should probably use its own
    // trait, instead of using `AsTraitObject` directly, so that implementers
    // are more clearly offering implementations for use with _that specific
    // abstraction_: a particular type might want to allow different trait
    // casts depending on what the resulting value is going to be used for.
    //
    // Each element of this vec effectively contains a pair of pointers, where
    // the first is to heap-allocated data for the object and the second is
    // to a static vtable for `AsTraitObject` that tells us which concrete
    // function to call when we're attempting a trait-to-trait cast, along with
    // how big the associated data is and how to drop it.
    let mut objs = Vec::<Box<dyn AsTraitObject>>::new();
    objs.push(Box::new(Implementer));
    objs.push(Box::new(NotImplementer));
    objs.push(Box::new(Implementer));
    objs.push(Box::new(NotImplementer));

    // This particular usage pattern is not realistic: in practice we'd
    // presumably take an index from an external caller and look up only one
    // one object associated with that handle and try to cast it. But this
    // still exercises all of the same machinery to make sure we can handle
    // both the implementing and non-implementing cases correctly.
    let results: Vec<Option<&str>> = objs
        .iter()
        .map(|h| {
            if let Some(with_msg) = cast_trait_object::<dyn WithMessage>(&**h) {
                Some(with_msg.message())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        &results,
        &[
            Some("hello from implementer"),
            None,
            Some("hello from implementer"),
            None
        ]
    );
}
