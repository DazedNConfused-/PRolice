//! Project's build info metadata, courtesy of the `built` crate.
//! See more: [https://docs.rs/built/0.4.4/built/](https://docs.rs/built/0.4.4/built/)

use log::LevelFilter;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

/// Return the project's default log-level.
pub fn default_log_level() -> LevelFilter {
    if built_info::DEBUG {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

/// Returns the project's main package name.
pub fn package_name() -> &'static str {
    built_info::PKG_NAME
}

/// Returns the project's full version.
pub fn full_version() -> &'static str {
    built_info::PKG_VERSION
}

/// Returns the project's colon-separated list of authors.
pub fn authors() -> &'static str {
    built_info::PKG_AUTHORS
}

/// Returns the project's description.
pub fn description() -> &'static str {
    built_info::PKG_DESCRIPTION
}
