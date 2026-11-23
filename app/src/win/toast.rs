//! Update toast (`UI.Notifications`): shown when a background check finds an update or one waits to be installed, cleared on exit.

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::core::{HSTRING, Result};

use crate::install::AUMID;
use crate::launch::ABOUT_URI;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Toast payload: the body opens About (`ABOUT_URI`), the protocol button launches `argument` (`UPDATE_URI` downloads, `ABOUT_URI` shows Install).
fn update_xml(text: &str, button: &str, argument: &str) -> String {
    format!(
        "<toast launch=\"{ABOUT_URI}\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>{}</text></binding></visual><actions><action content=\"{}\" arguments=\"{}\" activationType=\"protocol\"/></actions></toast>",
        escape(text),
        escape(button),
        escape(argument)
    )
}

/// Only reached from a Velopack install (the AUMID is registered there). Failures are logged and swallowed.
pub fn show_update(text: &str, button: &str, argument: &str) {
    let show = || -> Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(update_xml(text, button, argument)))?;
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
    use crate::launch::{ABOUT_URI, UPDATE_URI};

    #[test]
    fn update_toast_xml() {
        let xml = update_xml("A <new> \"version\" & more", "Download", UPDATE_URI);
        assert_eq!(
            xml,
            "<toast launch=\"dragon-imageviewer:about\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>A &lt;new&gt; &quot;version&quot; &amp; more</text></binding></visual><actions><action content=\"Download\" arguments=\"dragon-imageviewer:update\" activationType=\"protocol\"/></actions></toast>"
        );
    }

    #[test]
    fn ready_toast_xml() {
        let xml = update_xml("An update is available", "Show update", ABOUT_URI);
        assert_eq!(
            xml,
            "<toast launch=\"dragon-imageviewer:about\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>An update is available</text></binding></visual><actions><action content=\"Show update\" arguments=\"dragon-imageviewer:about\" activationType=\"protocol\"/></actions></toast>"
        );
    }
}
