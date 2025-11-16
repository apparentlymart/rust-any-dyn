//! Type-erased trait objects.
//!
//! The main purpose of this library is to provide a types that can be used to
//! represent trait objects for _any_ dyn-compatible trait, with the decision
//! of which trait being made at runtime. Refer to the [`AnyDyn`] documentation
//! for a usage example.
//!
//! There are various libraries that offer different strategies for
//! cross-converting between trait objects of different traits. This library
//! does not actually solve that problem, but it provides a building block that
//! can be useful when solving that problem: a meeting point where a function
//! that knows how to produce a trait object and a function that knows which
//! trait object type it wants can coordinate at runtime.
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
//! The current form of this trait relies on the unstable `ptr_metadata`
//! feature, and so can only work on nightly Rust. More importantly, it relies
//! on an implementation detail that is not actually guaranteed: that the
//! metadata for trait objects always has the same size and alignment regardless
//! of which trait is being implemented.
//!
//! If that implementation detail changes in future -- for example, if certain
//! traits have larger metadata in a future version of Rust -- then this library
//! will panic at runtime when constructing type-erased trait objects for
//! certain traits.
//!
//! Note that it depends only on all trait object metadata having the same
//! size and alignment; it does not depend on any specific representation of
//! that metadata. This trait will not be broken if the metadata representation
//! for _all_ trait object types changes together in a future language version.
//!
//! **If that situation bothers you, do not use this library**.
#![no_std]
#![feature(ptr_metadata)]

use core::{
    alloc::Layout,
    any::TypeId,
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::{DynMetadata, NonNull},
};

/// A shared reference to a trait object for an erased trait tracked only at
/// runtime.
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
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Cast returns a reference to a trait object of type `Dyn` if and only if
    /// this [`AnyDyn`] value was constructed from a trait object of the same
    /// type.
    #[inline]
    pub fn cast<Dyn: ?Sized + 'static>(&'a self) -> Option<&'a Dyn>
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        self.ptr.cast::<Dyn>().map(|ptr| unsafe {
            // Safety: AnyDynPtr guarantees that it will only return Some
            // if the following is safe.
            ptr.as_ref()
        })
    }
}

/// A mutable reference to a trait object for an erased trait tracked only at
/// runtime.
///
/// This is essentially the same as [`AnyDyn`] except that it represents a
/// mutable reference instead of a shared reference.
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
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// Cast returns a reference to a trait object of type `Dyn` if and only if
    /// this [`AnyDynMut`] value was constructed from a trait object of the same
    /// type.
    #[inline]
    pub fn cast<Dyn: ?Sized + 'static>(&'a self) -> Option<&'a mut Dyn>
    where
        Dyn: core::ptr::Pointee<Metadata = core::ptr::DynMetadata<Dyn>>,
    {
        self.ptr.cast::<Dyn>().map(|mut ptr| unsafe {
            // Safety: AnyDynPtr guarantees that it will only return Some
            // if the following is safe.
            ptr.as_mut()
        })
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
/// around this which follow the lifetime and mutability of the given references.
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
