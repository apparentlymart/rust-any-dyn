//! Type-erased trait objects.
//!
//! The main purpose of this library is to provide a types that can be used to
//! represent trait objects for _any_ dyn-compatible trait, with the decision
//! of which trait being made at runtime. Refer to the [`AnyDyn`] documentation
//! for a usage example.
//!
//! There are various libraries that offer different strategies for
//! cross-converting between trait objects of different traits. This library
//! _does not_ aim to be a batteries-included solution to that problem, but
//! it does offer a number of building blocks you can use to implement such
//! facilities for yourself as part of your own application.
//!
//! The [`traitcast`] module includes a simple trait-to-trait casting
//! implementation that you can use directly if you like, but it is mainly
//! intended as an example of how you might include such facilities as part of
//! broader abstractions within your own application.
//!
//! Because this erases both the concrete target type and the chosen trait type,
//! it can only work for trait objects for types that are `'static`, similar
//! to the constraints for [`core::any::Any`].
//!
//! This library depends only on `core`, so it can be used as a dependency of
//! `no_std` callers.
//!
//! # WARNING: This relies on Rust implementation details!
//!
//! The current implementation of type-erased trait object references relies on
//! the unstable `ptr_metadata` feature, and so can only work on nightly Rust.
//! More importantly, it relies on a specific implementation detail that is not
//! actually guaranteed at the time of writing: that the metadata for trait
//! objects always has the same size and alignment regardless of which trait is
//! being implemented.
//!
//! If that implementation detail changes in future -- for example, if certain
//! traits have larger metadata in a future version of Rust -- then this library
//! will panic at runtime when constructing type-erased trait objects for
//! certain traits, but it should not cause undefined behavior.
//!
//! Note that it depends only on all trait object metadata having the same
//! size and alignment; it does _not_ depend on any specific representation of
//! that metadata. This trait will not be broken if the metadata representation
//! for _all_ trait object types changes together in a future language version.
//!
//! This library will keep using `0.*.*` version numbers at least until
//! it's not relying on unstable features or implementation details. Due to
//! relying on unstable and unspecified details, future versions of Rust could
//! potentially break this library in a way that cannot be repaired.
//!
//! **If that situation bothers you, then do not use this library!**
#![no_std]
#![feature(ptr_metadata)]

use core::{
    alloc::Layout,
    any::TypeId,
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::{DynMetadata, NonNull},
};

pub mod traitcast;

/// A shared reference to a trait object for an erased trait tracked only at
/// runtime.
///
/// In other words, this is like `&'a dyn Trait`, but with `Trait` tracked
/// dynamically instead of statically,
///
/// ```
/// # use any_dyn::AnyDyn;
/// trait ExampleTrait {
///     fn message(&self) -> &'static str;
/// }
///
/// struct ExampleImpl;
///
/// impl ExampleTrait for ExampleImpl {
///     fn message(&self) -> &'static str {
///         "Hello, world!"
///     }
/// }
///
/// let ei = ExampleImpl;
/// let erased: AnyDyn = AnyDyn::new(&ei as &dyn ExampleTrait);
/// // `erased` is now equivalent to a &dyn ExampleTrait value except that
/// // the trait it implements is also erased, in addition to the normal erasure
/// // of the underlying implementation type.
/// //
/// // To recover a &dyn ExampleTrait again the caller needs to know which
/// // trait it's expecting.
/// if let Some(et) = erased.cast::<dyn ExampleTrait>() {
///     println!("The message is {:?}", et.message());
/// }
/// ```
///
/// This type represents only shared (immutable) references to trait objects.
/// Consider [`AnyDynMut`] if you need an exclusive mutable reference.
#[derive(Debug, Clone, Copy)]
pub struct AnyDyn<'a> {
    ptr: AnyDynPtr,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> AnyDyn<'a> {
    /// Creates an [`AnyDyn`] value that represents the same trait object
    /// given in `from`, but with the specific trait erased as runtime data
    /// instead of part of the result type.
    ///
    /// Callers can recover `from` by calling [`AnyDyn::cast`] with the
    /// same trait object type.
    #[inline]
    pub fn new<Dyn: ?Sized + 'static>(from: &'a Dyn) -> Self
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        let ptr = AnyDynPtr::new(NonNull::from(from));
        // Safety: We're returning with the same lifetime we were given.
        unsafe { Self::from_raw(ptr) }
    }

    /// Conjures a an [`AnyDyn`] with an arbitrary lifetime from an
    /// [`AnyDynPtr`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that the resulting lifetime is correct for
    /// the object behind the given pointer.
    #[inline]
    pub const unsafe fn from_raw(ptr: AnyDynPtr) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Cast returns a reference to a trait object of type `Dyn` if and only if
    /// this [`AnyDyn`] value was constructed from a trait object of the same
    /// type.
    #[inline]
    pub fn cast<Dyn: ?Sized + 'static>(self) -> Option<&'a Dyn>
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        self.ptr.cast::<Dyn>().map(|ptr| unsafe {
            // Safety: AnyDynPtr guarantees that it will only return Some
            // if the following is safe.
            ptr.as_ref()
        })
    }

    /// Returns the underlying [`AnyDynPtr`] for this trait object reference.
    #[inline]
    pub const fn as_ptr(self) -> AnyDynPtr {
        self.ptr
    }
}

/// A mutable reference to a trait object for an erased trait tracked only at
/// runtime.
///
/// This is essentially the same as [`AnyDyn`] except that it represents a
/// mutable reference instead of a shared reference.
///
/// In other words, this is like `&'a mut dyn Trait`, but with `Trait` tracked
/// dynamically instead of statically,
#[derive(Debug, Clone, Copy)]
pub struct AnyDynMut<'a> {
    ptr: AnyDynPtr,
    _phantom: PhantomData<&'a mut ()>,
}

impl<'a> AnyDynMut<'a> {
    /// Creates an [`AnyDynMut`] value that represents the same trait object
    /// given in `from`, but with the specific trait erased as runtime data
    /// instead of part of the result type.
    ///
    /// Callers can recover `from` by calling [`AnyDynMut::cast`] with the
    /// same trait object type.
    #[inline]
    pub fn new<Dyn: ?Sized + 'static>(from: &'a mut Dyn) -> Self
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        let ptr = AnyDynPtr::new(NonNull::from(from));
        // Safety: We're returning with the same lifetime we were given.
        unsafe { Self::from_raw(ptr) }
    }

    /// Conjures a an [`AnyDynMut`] with an arbitrary lifetime from an
    /// [`AnyDynPtr`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that the resulting lifetime is correct for
    /// the object behind the given pointer.
    #[inline]
    pub const unsafe fn from_raw(ptr: AnyDynPtr) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Cast returns a reference to a trait object of type `Dyn` if and only if
    /// this [`AnyDynMut`] value was constructed from a trait object of the same
    /// type.
    #[inline]
    pub fn cast<Dyn: ?Sized + 'static>(self) -> Option<&'a mut Dyn>
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        self.ptr.cast::<Dyn>().map(|mut ptr| unsafe {
            // Safety: AnyDynPtr guarantees that it will only return Some
            // if the following is safe.
            ptr.as_mut()
        })
    }

    /// Returns the underlying [`AnyDynPtr`] for this trait object reference.
    #[inline]
    pub const fn as_ptr(self) -> AnyDynPtr {
        self.ptr
    }
}

/// A non-null raw pointer to a trait object for an arbitrary trait decided at
/// runtime.
///
/// A value of this type is similar to a `NonNull<dyn Trait>`, but with the
/// specific trait erased so it can vary at runtime.
///
/// This is the raw pointer version of [`AnyDyn`] and [`AnyDynMut`], which
/// therefore does not track any lifetimes. Those other two types are wrappers
/// around this which track the lifetime and mutability of the underlying
/// object.
#[derive(Debug, Clone, Copy)]
pub struct AnyDynPtr {
    thin: NonNull<()>,
    metadata: MaybeUninit<DynMetadata<()>>,
    type_id: TypeId,
}

impl AnyDynPtr {
    /// Creates an [`AnyDynPtr`] value that represents the same trait object
    /// given in `from`, but with the specific trait erased as runtime data
    /// instead of part of the result type.
    ///
    /// Callers can recover `from` by calling [`AnyDynPtr::cast`] with the
    /// same trait object type.
    pub fn new<Dyn: ?Sized + 'static>(from: NonNull<Dyn>) -> Self
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        // The following is to make it more likely that we'll notice quickly
        // if the implementation detail we're relying on changes in a future
        // version of Rust. Note that we're only depending on equal layout and
        // not on identical representation, because we never actually try to
        // access the target data as the erased type.
        assert_eq!(
            const { Layout::new::<DynMetadata<Dyn>>() },
            const { Layout::new::<DynMetadata<()>>() },
            "DynMetadata types no longer have fixed layout regardless of type parameter",
        );

        let thin = from.cast::<()>();
        let metadata = core::ptr::metadata(from.as_ptr());
        let type_id = core::any::TypeId::of::<Dyn>();

        // We copy the metadata verbatim into an opaque container whose
        // layout matches `DynMetadata<()>`, but we never actually access
        // it as that type: we'll turn this back into DynMetadata<Dyn>
        // again before we actually try to make use of it.
        let mut erased_metadata = MaybeUninit::<DynMetadata<()>>::uninit();
        let erased_metadata_ptr = erased_metadata.as_mut_ptr();
        let our_metadata_erased = &metadata as *const DynMetadata<Dyn> as *const DynMetadata<()>;
        unsafe {
            core::ptr::copy_nonoverlapping(our_metadata_erased, erased_metadata_ptr, 1);
        }

        Self {
            thin,
            metadata: erased_metadata,
            type_id,
        }
    }

    /// Cast returns a pointer to a trait object of type `Dyn` if and only if
    /// this [`AnyDynPtr`] value was constructed from a trait object of the same
    /// type.
    #[inline]
    pub fn cast<Dyn: ?Sized + 'static>(&self) -> Option<NonNull<Dyn>>
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        if core::any::TypeId::of::<Dyn>() != self.type_id {
            return None;
        }
        let metadata_ptr = self.metadata.as_ptr() as *const core::ptr::DynMetadata<Dyn>;
        let metadata = unsafe {
            // Safety: If this object was constructed correctly then our
            // erased metadata is for the requested trait object type.
            core::ptr::read(metadata_ptr)
        };
        let ptr = core::ptr::from_raw_parts_mut::<Dyn>(self.thin.as_ptr(), metadata);
        Some(unsafe {
            // Safety: ptr was built from a NonNull<()>, so is definitely not
            // null itself.
            NonNull::new_unchecked(ptr)
        })
    }
}

/// Unique identifier for a `dyn Trait` trait object type.
///
/// This serves the same purpose as (and the similar limitations as)
/// [`core::any::TypeId`], but provides the additional guarantee that it
/// can only be safely constructed to represent trait object types and not
/// any other type.
///
/// This is intended for use in a trait-casting handshake protocol involving
/// a function that takes a `DynTypeId` and returns an optional [`AnyDyn`]
/// or [`AnyDynMut`] for the requested trait object type. The macro
/// [`traitcast::match_dyn_type_id`] can help with implementing such a function.
///
/// The following example shows a potential application involving polymorphic
/// "handle" values that can be tested to see if they implement specific traits
/// at runtime.
///
/// ```
/// # #![feature(ptr_metadata)]
/// # use any_dyn::{DynTypeId, AnyDyn};
/// pub trait WithMessage {
///     fn message(&self) -> &'static str;
/// }
///
///
/// pub trait WithIndex {
///     fn index(&self) -> usize;
/// }
///
/// // Represents an opaque handle to something which implements zero or more
/// // of the other traits.
/// pub trait Handle {
///     /// Returns a trait object for the given trait object type, if and only
///     /// the implementer is able and willing to act as an implementation of
///     /// that trait.
///     //
///     // This uses `DynTypeId` rather than a generic type argument so
///     // that this trait is dyn-compatible.
///     fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
///         None // default implementation supports no traits at all
///     }
/// }
///
/// /// Generic-typed helper for using [`Handle`] more conveniently to
/// /// attempt to cast an arbitrary handle object into a different trait
/// /// object.
/// pub fn cast_handle<'a, Dyn: ?Sized + 'static>(hnd: &'a dyn Handle) -> Option<&'a Dyn>
/// where
///    Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
/// {
///     let any = hnd.as_trait_object(DynTypeId::of::<Dyn>())?;
///     any.cast::<Dyn>()
/// }
///
/// struct HelloWorld;
///
/// impl WithMessage for HelloWorld {
///     fn message(&self) -> &'static str {
///         "Hello, world!"
///     }
/// }
///
/// impl Handle for HelloWorld {
///     fn as_trait_object<'a>(&'a self, type_id: DynTypeId) -> Option<AnyDyn<'a>> {
///         // Each that each type must explicitly enumerate which traits
///         // it intends to support through this API, so the implementer
///         // has control of what subset of their implemented traits they
///         // want to expose through the "Handle" abstraction.
///         //
///         // This particular handle type only supports `WithMessage`.
///         if type_id == DynTypeId::of::<dyn WithMessage>() {
///             Some(AnyDyn::new(self as &dyn WithMessage))
///         } else {
///             None
///         }
///
///         // NOTE: The above is a hand-written implementation just to
///         // illustrate how these different parts can fit together, but
///         // the macro `match_dyn_type_id` in the `traitcast` module
///         // can help generate code like the above automatically.
///     }
/// }
///
/// fn usage_example() {
///     let hello_world = HelloWorld;
///     let hnd: &dyn Handle = &hello_world;
///     // Imagine that `hnd` had been stashed in an element of hetrogenous
///     // `Vec<Box<dyn Handle>>`, or similar, and we just looked it up by index
///     // to serve a dynamic request from outside of the current process that
///     // needs to return a message, if and only if the handle happens to
///     // implement that trait...
///     if let Some(with_message) = cast_handle::<dyn WithMessage>(hnd) {
///         println!("message is {:?}", with_message.message());
///     }
///     /// ...or an index, if it happens to implement that trait...
///     if let Some(with_index) = cast_handle::<dyn WithIndex>(hnd) {
///         println!("index is {:?}", with_index.index());
///     }
/// }
/// # usage_example();
/// ```
///
/// If you want to do something like this and are relatively unopinionated
/// about the details, you might find the symbols in [`traitcast`] useful.
/// However, these building blocks are intended to allow you to build your
/// own specialized versions of those helpers, if e.g. you want to include it
/// as part of a larger abstraction.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynTypeId {
    type_id: TypeId,
}

impl DynTypeId {
    /// Returns the [`DynTypeId`] of the type parameter `Dyn`, which must be
    /// a trait object type.
    #[inline]
    pub const fn of<Dyn: ?Sized + 'static>() -> Self
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        Self {
            type_id: core::any::TypeId::of::<Dyn>(),
        }
    }
}
