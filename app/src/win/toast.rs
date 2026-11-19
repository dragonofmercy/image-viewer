//! Update toast (`UI.Notifications`): shown when a background check finds an update, cleared on exit.

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::core::{HSTRING, Result};

use crate::install::AUMID;
use crate::launch::UPDATE_URI;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Toast payload: a text and one protocol button, both activating `UPDATE_URI`.
fn update_xml(text: &str, button: &str) -> String {
    format!(
        "<toast launch=\"{UPDATE_URI}\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>{}</text></binding></visual><actions><action content=\"{}\" arguments=\"{UPDATE_URI}\" activationType=\"protocol\"/></actions></toast>",
        escape(text),
        escape(button)
    )
}

/// Only reached from a Velopack install (the AUMID is registered there). Failures are logged and swallowed.
pub fn show_update(text: &str, button: &str) {
    let show = || -> Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(update_xml(text, button)))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&toast)
    };
    if let Err(e) = show() {
        eprintln!("cannot show the update toast: {e}");
    }
}

/// Removes our toasts from the action center. Runs on every exit, so an unknown identity (not installed) is logged once at most.
pub fn clear() {
    static LOGGED: AtomicBool = AtomicBool::new(false);
    let clear = || ToastNotificationManager::History()?.ClearWithId(&HSTRING::from(AUMID));
    if let Err(e) = clear()
        && !LOGGED.swap(true, Ordering::Relaxed)
    {
        eprintln!("cannot clear the update toast: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::update_xml;

    #[test]
    fn update_toast_xml() {
        let xml = update_xml("A <new> \"version\" & more", "Download");
        assert_eq!(
            xml,
            "<toast launch=\"dragon-imageviewer:update\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>A &lt;new&gt; &quot;version&quot; &amp; more</text></binding></visual><actions><action content=\"Download\" arguments=\"dragon-imageviewer:update\" activationType=\"protocol\"/></actions></toast>"
        );
    }
}
