codes! {
    /// Raised when the free space holds too little content at the chosen quality.
    HEADROOM_LOW = "QUAL-1" {
        severity: Warning,
        status: 500,
        since: "0.4.0",
        meaning: "The free space is thin for the chosen preset — it holds only a few hours of \
            content at that quality. Nothing is broken and nothing already downloaded is \
            affected; new acquisitions will simply fill the disk quickly.",
        remedy: "Free space, move the data location to a larger volume, or choose a lighter \
            preset for the media that does not need it.",
    }
    /// Raised when releases exist but the profile — the chosen quality included — wants
    /// none of them.
    PRESET_UNMET = "QUAL-2" {
        severity: Warning,
        status: 500,
        since: "0.4.0",
        meaning: "Releases exist for wanted content and the chosen preset wants none of them. \
            The indexer is working; the preset is stricter than what can be found.",
        remedy: "Choose a less demanding preset for that media, or wait for a matching release. \
            The content stays wanted either way.",
    }
    /// Raised when a clean search turns up nothing at all for wanted content.
    NONE_AVAILABLE = "QUAL-3" {
        severity: Warning,
        status: 500,
        since: "0.4.0",
        meaning: "A clean search turned up nothing at all for wanted content. The indexer \
            answered — this is not an indexer failure — there is simply nothing to grab yet.",
        remedy: "Check the indexer carries this content, or wait. No action is needed if it is \
            merely not out yet.",
    }
}
