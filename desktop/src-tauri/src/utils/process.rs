//! Process helpers.
//!
//! Provides a single place to configure background child processes so they do
//! not flash transient console windows on Windows.

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
pub fn configure_background_process(
    cmd: &mut tokio::process::Command,
) -> &mut tokio::process::Command {
    use std::os::windows::process::CommandExt;

    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn configure_background_process(
    cmd: &mut tokio::process::Command,
) -> &mut tokio::process::Command {
    cmd
}

#[cfg(windows)]
pub fn configure_background_std_process(
    cmd: &mut std::process::Command,
) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;

    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn configure_background_std_process(
    cmd: &mut std::process::Command,
) -> &mut std::process::Command {
    cmd
}
