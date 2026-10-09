//! [`contract!`]: one capability's contract, declared once as the port methods it carries.
//!
//! A declaration names the capability, its major, and for each port trait the methods the
//! capability carries. Each argument is written with its type as the port declares it,
//! and marked with how it crosses:
//!
//! | Written | It crosses as |
//! |---|---|
//! | `str name: &str` | `String` |
//! | `opt_str name: Option<&str>` | `Option<String>` |
//! | `slice name: &[T] as T` | `Vec<T>` |
//! | `refer name: &T as T` | `T` |
//! | `value name: T` | `T` |
//!
//! An operation whose answer may be larger than [`crate::client::LARGEST`] says so after
//! its return type, as `where largest = BYTES`.
//!
//! The port's own type is written out, rather than derived from the mark, because the
//! trait implementations go through `#[async_trait]`, which gives every reference in a
//! signature its own lifetime and can only see a reference written as a type.
//!
//! It expands to a module holding, per operation, the request it crosses as; `Fills`, every
//! listed trait at once; an `Adapter` that implements every listed trait by asking an
//! adapter; `dispatch`, which serves a
//! call to anything implementing those traits; and `capability()`, the descriptor the
//! published documents are generated from.

/// Declare one capability's contract. See the module documentation for the marks.
///
/// Crate-private: a capability is declared here and nowhere else, so the core, the
/// adapter kit and the published documents all read the same declaration.
macro_rules! contract {
    (
        $(#[$meta:meta])*
        $vis:vis mod $module:ident = $capability:literal @ $major:literal {
            $(
                impl $Trait:path {
                    $(
                        $(#[$op_meta:meta])*
                        fn $op:ident( $( $mark:ident $arg:ident : $param:ty $(as $wire:ty)? ),* $(,)? ) -> $ret:ty $(where largest = $largest:expr)?;
                    )*
                }
            )*
        }
    ) => {
        $(#[$meta])*
        $vis mod $module {
            #[allow(unused_imports)]
            use super::*;

            /// The capability, as the vocabulary names it.
            pub const CAPABILITY: &str = $capability;

            /// The major of its contract this build speaks.
            pub const MAJOR: u32 = $major;

            $($(
                $(#[$op_meta])*
                pub mod $op {
                    #[allow(unused_imports)]
                    use super::*;

                    /// What the operation is asked with.
                    #[derive(
                        ::serde::Serialize,
                        ::serde::Deserialize,
                        ::schemars::JsonSchema,
                    )]
                    #[serde(deny_unknown_fields)]
                    pub struct Asked {
                        $(
                            #[doc = concat!("The port's `", stringify!($arg), "`.")]
                            pub $arg: $crate::contract!(@wire $mark $param $(, $wire)?),
                        )*
                    }
                }
            )*)*

            /// Whatever fills this capability: every port its contract carries.
            pub trait Fills: Send + Sync $( + $Trait )* {}

            impl<T> Fills for T where T: Send + Sync $( + $Trait )* {}

            /// An adapter that fills this capability, asked over its contract.
            #[derive(Debug, Clone)]
            pub struct Adapter(pub $crate::Contracted);

            $(
                #[::async_trait::async_trait]
                impl $Trait for Adapter {
                    $(
                        async fn $op(
                            &self,
                            $( $arg: $param ),*
                        ) -> ::core::result::Result<$ret, $crate::Failure> {
                            let asked = $op::Asked {
                                $( $arg: $crate::contract!(@to_wire $mark $arg), )*
                            };
                            self.0
                                .call(
                                    CAPABILITY,
                                    MAJOR,
                                    stringify!($op),
                                    $crate::contract!(@largest $($largest)?),
                                    &asked,
                                )
                                .await
                        }
                    )*
                }
            )*

            /// Serve one call of this capability's contract from `adapter`.
            ///
            /// # Errors
            ///
            /// The refusal for an operation nobody declared, for a body that does not
            /// read as its request, and for a failure the port answered.
            pub async fn dispatch<A>(
                adapter: &A,
                operation: &str,
                body: &[u8],
            ) -> ::core::result::Result<::std::vec::Vec<u8>, $crate::Refusal>
            where
                A: Send + Sync $( + $Trait )*,
            {
                $($(
                    if operation == stringify!($op) {
                        #[allow(unused_variables)]
                        let asked: $op::Asked = $crate::wire::asked(body)?;
                        let answer = <A as $Trait>::$op(
                            adapter,
                            $( $crate::contract!(@borrow $mark asked.$arg) ),*
                        )
                        .await;
                        return $crate::wire::answered(answer);
                    }
                )*)*
                Err($crate::Refusal::unknown_operation(operation))
            }

            /// This capability, served from `adapter` by an adapter kit.
            #[must_use]
            pub fn served<A>(adapter: ::std::sync::Arc<A>) -> $crate::Served
            where
                A: Send + Sync + 'static $( + $Trait )*,
            {
                $crate::Served::new(CAPABILITY, MAJOR, move |operation, body| {
                    let adapter = ::std::sync::Arc::clone(&adapter);
                    ::std::boxed::Box::pin(async move {
                        dispatch(&*adapter, &operation, &body).await
                    })
                })
            }

            /// This capability's contract: its name, its major and every operation.
            #[must_use]
            pub fn capability() -> $crate::Capability {
                $crate::Capability {
                    name: CAPABILITY,
                    major: MAJOR,
                    operations: vec![
                        $($(
                            $crate::Operation::new::<$op::Asked, $ret>(
                                CAPABILITY,
                                MAJOR,
                                stringify!($op),
                            )
                            .within($crate::contract!(@largest $($largest)?)),
                        )*)*
                    ],
                }
            }
        }
    };

    (@largest) => { $crate::client::LARGEST };
    (@largest $largest:expr) => { $largest };

    (@wire str $p:ty) => { ::std::string::String };
    (@wire opt_str $p:ty) => { ::core::option::Option<::std::string::String> };
    (@wire slice $p:ty, $w:ty) => { ::std::vec::Vec<$w> };
    (@wire refer $p:ty, $w:ty) => { $w };
    (@wire value $p:ty) => { $p };

    (@to_wire str $a:ident) => { $a.to_owned() };
    (@to_wire opt_str $a:ident) => { $a.map(str::to_owned) };
    (@to_wire slice $a:ident) => { $a.to_vec() };
    (@to_wire refer $a:ident) => { $a.clone() };
    (@to_wire value $a:ident) => { $a };

    (@borrow str $e:expr) => { &$e };
    (@borrow opt_str $e:expr) => { $e.as_deref() };
    (@borrow slice $e:expr) => { &$e };
    (@borrow refer $e:expr) => { &$e };
    (@borrow value $e:expr) => { $e };
}

pub(crate) use contract;
