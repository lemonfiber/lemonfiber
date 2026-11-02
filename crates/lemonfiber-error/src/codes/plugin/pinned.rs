//! The `PLUGIN` codes of a plugin held against the parts this build pins.

codes! {
    /// Raised when a plugin that is not first-party declares a service replacing one the
    /// stack ships.
    STRANGER_REPLACING = "PLUGIN-40" {
        severity: Error,
        status: 403,
        since: "0.18.0",
        meaning: "The plugin declares a service that moves one of the stack's own services in \
            place, and only a plugin this build embeds as first-party may do that. Nothing was \
            written.",
        remedy: "Install the plugin without that service's replaces, or use the first-party \
            plugin this build names for that part.",
    }
    /// Raised when a plugin carrying the id of a part this build pins comes from anywhere
    /// but that pin.
    ANOTHER_SOURCE = "PLUGIN-49" {
        severity: Error,
        status: 403,
        since: "0.18.0",
        meaning: "The plugin's id is one this build pins and carries, and what was offered is \
            not the tree the pin names: a path, a checkout, or a release the pin does not \
            name. Nothing was written.",
        remedy: "Install it by its id alone, from the tree this build carries, or give a plugin \
            of your own an id of its own.",
    }
}
