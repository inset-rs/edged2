//! The user's browser.

/// Opens `url` in the user's default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()?;
    Ok(())
}
