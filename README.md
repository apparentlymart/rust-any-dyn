# `any_trait`: trait-to-trait casting utilities for Rust

This Rust library contains some building blocks for building systems that need
to be able to cast from one trait object type to another.

The two main building blocks are some types that represent trait objects with
the specific trait erased and tracked only at runtime (`Dyn`, `DynMut`), and
opaque values that can be used to identify a trait object for a specific trait
(`DynTypeId`).

With those two building blocks it's possible to write a method that takes a
trait object type ID and returns a trait-erased trait object, which can then be
downcast into a real trait object for the selected trait.

There's also a lightweight example implementation of implementing trait-to-trait
casting using those building blocks, in module `traitcast`.
