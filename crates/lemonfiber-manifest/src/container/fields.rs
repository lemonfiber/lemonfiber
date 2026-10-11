//! The container fields themselves, declared once and written into each manifest's
//! service.

/// A service struct with the container fields appended to the fields it declares.
#[macro_export]
macro_rules! container_fields {
    ($(#[$meta:meta])* $vis:vis struct $name:ident { $($own:tt)* }) => {
        $(#[$meta])*
        $vis struct $name {
            $($own)*
            /// Each further port it publishes on its tier.
            #[serde(default)]
            pub publish: ::std::vec::Vec<$crate::container::Publish>,
            /// Whether it takes the data mount.
            #[serde(default)]
            pub takes_data: bool,
            /// Where inside the container its one configuration directory is mounted,
            /// `/config` where absent.
            #[serde(default)]
            pub config_path: ::std::option::Option<::std::string::String>,
            /// Whether its configuration directory is mounted read-only.
            #[serde(default)]
            pub config_ro: bool,
            /// Each further path mounted from its own mount tree.
            #[serde(default)]
            pub mount: ::std::vec::Vec<$crate::container::Mount>,
            /// Absolute paths given scratch space in memory.
            #[serde(default)]
            pub tmpfs: ::std::vec::Vec<::std::string::String>,
            /// Its environment, each variable one of four typed kinds of value.
            #[serde(default)]
            pub env: ::std::collections::BTreeMap<::std::string::String, $crate::container::Value>,
            /// Who the container runs as.
            #[serde(default)]
            pub user_mode: ::std::option::Option<$crate::container::UserMode>,
            /// How tightly the container is held.
            #[serde(default)]
            pub posture: $crate::container::Posture,
            /// The most memory the container may hold, in MiB.
            #[serde(default)]
            pub mem_limit_mib: ::std::option::Option<u32>,
            /// The capability whose filler's network it runs inside.
            #[serde(default)]
            pub network: ::std::option::Option<$crate::container::Network>,
            /// The one privileged shape lemonfiber writes for it, from a closed set.
            #[serde(default)]
            pub shape: ::std::option::Option<$crate::container::Shape>,
        }
    };
}
