//! Some optional additional helpers for implementing trait-to-trait casting
//! for trait objects.
//!
//! This library's main goal is to provide some low-level building blocks that
//! callers can use to implement their own trait-to-trait object casting
//! facilities, but this module contains some general-purpose facilities for
//! that which you can either use directly or treat as an example for how you
//! might build something similar yourself.
//!
//! If you'd like to learn more, start with [`AsTraitObject`].

use crate::{AnyDyn, AnyDynMut, DynTypeId};

/// A `dyn`-compatible trait used by [`cast_trait_object`] to find out whether
/// an implementer wishes to support casting to a trait object of a different
/// type and, if so, to get a type-erased trait object for that trait.
///
/// Refer to [`match_dyn_type_id`] for an example of how to implement this
/// trait using that macro, or on how you can write the equivalent code out
/// directly yourself.
///
/// If you want to use trait object casting as part of a broader abstraction
/// where callers would start with a trait object for your own different
/// `dyn`-compatible trait then you can add a method just like
/// [`AsTraitObject::as_trait_object`] to your own trait instead of using
/// this one, and provide your own version of [`cast_trait_object`] that
/// works with trait objects of your own trait. This trait is here mainly
/// just as an example of how to use this library's facilities to implement
/// trait-to-trait casting.
pub trait AsTraitObject {
    /// Returns a type-erased trait object for the type identified by `type_id`
    /// if and only if the implementer wishes to offer an implementation of
    /// the associated trait.
    ///
    /// Callers should typically use [`cast_trait_object`] instead of calling
    /// this method directly, if they can statically specify which trait object
    /// type they are interested in.
    ///
    /// Implementations of this can typically use [`match_dyn_type_id`] to
    /// perform the appropriate type matching and [`AnyDyn`] construction.
    #[inline]
    fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
        let _ = type_id;
        None
    }
}

/// Dynamically cast any [`AsTraitObject`] implementer to an arbitrary trait
/// object type, if and only if the implementer chooses to offer an
/// implementation of that trait.
///
/// ```
/// # use any_dyn::{
/// #     AnyDyn,
/// #     DynTypeId,
/// #     traitcast::{
/// #         AsTraitObject,
/// #         cast_trait_object,
/// #         match_dyn_type_id,
/// #     },
/// # };
/// # trait SomeTrait { fn some_trait_method(&self) {} }
/// # struct SomeStruct {}
/// # impl SomeTrait for SomeStruct {}
/// # impl AsTraitObject for SomeStruct {
/// #     fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
/// #         match_dyn_type_id!(self, type_id => SomeTrait)
/// #     }
/// # }
/// #
/// let concrete = SomeStruct {};
/// let as_trait_object = &concrete as &dyn AsTraitObject;
/// //
/// // ...then store `as_trait_object` somewhere that "forgets" its original
/// // concrete type, and then subsequently pull it back out and...
/// //
/// if let Some(trait_obj) = cast_trait_object::<dyn SomeTrait>(as_trait_object) {
///     // Call SomeTrait::some_trait_method only if the type chose to
///     // allow casting to that trait.
///     trait_obj.some_trait_method();
/// }
/// ```
///
/// Refer to [`AsTraitObject`] for more information. This is really just a
/// thin wrapper around [`AsTraitObject::as_trait_object`] followed by calling
/// [`AnyDyn::cast`] on its result, but typically more convenient to use because
/// the trait object type only needs to be written once and the intermediate
/// [`AnyDyn`] representation is encapsulated.
#[inline]
pub fn cast_trait_object<Dyn: ?Sized + 'static>(obj: &dyn AsTraitObject) -> Option<&Dyn>
where
    Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
{
    let any = obj.as_trait_object(DynTypeId::of::<Dyn>())?;
    any.cast::<Dyn>()
}

#[doc(hidden)]
#[macro_export]
macro_rules! __match_dyn_type_id {
    ($self:expr, $type_id:expr => $($trait_n:path),+ ) => {{
        type DynTypeId = $crate::DynTypeId;
        let type_id: $crate::DynTypeId = $type_id;
        let v: &_ = $self;
        let ret: Option<$crate::AnyDyn> = if false {
            _ = type_id;
            None
        }
        $(
        else if $type_id == DynTypeId::of::<dyn $trait_n>() {
            Some($crate::AnyDyn::new($self as &dyn $trait_n))
        }
        )+
        else {
            None
        };
        ret
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __match_dyn_type_id_mut {
    ($self:expr, $type_id:expr => $($trait_n:path),+ ) => {{
        type DynTypeId = $crate::DynTypeId;
        let type_id: $crate::DynTypeId = $type_id;
        let v: &_ = $self;
        let ret: Option<$crate::AnyDynMut> = if false {
            _ = type_id;
            None
        }
        $(
        else if $type_id == DynTypeId::of::<dyn $trait_n>() {
            Some($crate::AnyDynMut::new($self as &mut dyn $trait_n))
        }
        )+
        else {
            None
        };
        ret
    }};
}

/// Helper for implementing lookups from [`DynTypeId`] to [`AnyDyn`] for
/// a specified set of traits.
///
/// Implementations of [`AsTraitObject::as_trait_object`], or of any similar
/// method of your own trait that supports dynamic trait object casting,
/// can use this to concisely declare which traits they intend to support.
///
/// ```
/// use any_dyn::traitcast::{AsTraitObject, match_dyn_type_id};
/// use any_dyn::{AnyDyn, DynTypeId};
///
/// trait SomeTrait { /* ... */ }
/// trait SomeOtherTrait { /* ... */ }
///
/// struct SomeStruct { /* ... */ }
/// impl SomeTrait for SomeStruct { /* ... */ }
/// impl SomeOtherTrait for SomeStruct { /* ... */ }
///
/// impl AsTraitObject for SomeStruct {
///     fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
///         // The macro expands to an expression that returns Option<AnyDyn>.
///         match_dyn_type_id!(self, type_id => SomeTrait, SomeOtherTrait)
///     }
/// }
/// ```
///
/// `self` must be a reference to a type that implements all of the listed
/// traits, all of which be `dyn`-compatible and `'static`. `type_id` must
/// be a value of type `DynTypeId`.
///
/// The generated code is essentially just a chain of `if`/`else if`/`else`
/// statements comparing the given `type_id` with each of the listed traits
/// in turn. For example, the `match_dyn_type_id!` call in the above expands
/// to something functionally equivalent to the following:
///
/// ```
/// # use any_dyn::{AnyDyn, DynTypeId};
/// # trait SomeTrait {}
/// # trait SomeOtherTrait {}
/// # struct SomeStruct {}
/// # impl SomeTrait for SomeStruct {}
/// # impl SomeOtherTrait for SomeStruct {}
/// # impl SomeStruct {
/// # fn example<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
/// if type_id == DynTypeId::of::<dyn SomeTrait>() {
///     Some(AnyDyn::new(self as &dyn SomeTrait))
/// } else if type_id == DynTypeId::of::<dyn SomeOtherTrait>() {
///     Some(AnyDyn::new(self as &dyn SomeOtherTrait))
/// } else {
///     None
/// }
/// # }
/// # }
/// ```
///
/// You are welcome to hand-write similar code yourself if you prefer. This
/// macro is just a convenience helper to help focus on just listing which
/// traits are supported, rather than exposing the implementation details.
#[doc(inline)]
pub use __match_dyn_type_id as match_dyn_type_id;

/// Helper for implementing lookups from [`DynTypeId`] to [`AnyDynMut`] for
/// a specified set of traits.
///
/// This is just like [`match_dyn_type_id`] except that it produces
/// [`AnyDynMut`] results instead of [`AnyDyn`], and must therefore be used
/// with a mutable reference to something that implements each of the listed
/// traits.
///
/// ```
/// # use any_dyn::traitcast::match_dyn_type_id_mut;
/// # use any_dyn::{AnyDynMut, DynTypeId};
/// # trait SomeTrait { /* ... */ }
/// # trait SomeOtherTrait { /* ... */ }
/// # struct SomeStruct { /* ... */ }
/// # impl SomeTrait for SomeStruct { /* ... */ }
/// # impl SomeOtherTrait for SomeStruct { /* ... */ }
/// # trait AsTraitObjectMut {
/// #   fn as_trait_object_mut<'a>(&'a mut self, type_id: DynTypeId) -> Option<AnyDynMut<'a>>;
/// # }
/// // (AsTraitObjectMut is not a real trait in this library: it's just an
/// // example of a hypothetical mutable-trait-object trait you could
/// // specify yourself if you need something like it, and then implement it
/// // with the help of this macro.)
/// impl AsTraitObjectMut for SomeStruct {
///     fn as_trait_object_mut<'a>(&'a mut self, type_id: DynTypeId) -> Option<AnyDynMut<'a>> {
///         // The macro expands to an expression that returns Option<AnyDynMut>.
///         match_dyn_type_id_mut!(self, type_id => SomeTrait, SomeOtherTrait)
///     }
/// }
/// ```
///
/// `self` must be a mutable reference to a type that implements all of the
/// listed traits, all of which be `dyn`-compatible and `'static`. `type_id`
/// must be a value of type `DynTypeId`.
///
/// Refer to [`match_dyn_type_id`] for a more complete example. As with that
/// macro, you can write an equivalent `if`/`else if`/`else` sequence yourself
/// directly if you prefer.
#[doc(inline)]
pub use __match_dyn_type_id_mut as match_dyn_type_id_mut;

#[expect(unused)]
type AnyDynMutUsed<'a> = AnyDynMut<'a>;
