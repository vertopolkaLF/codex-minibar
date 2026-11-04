//! Refresh quota snapshots after suspend or hibernation, through logind/elogind.

use std::sync::mpsc::Sender;

use crate::streamdeck::Command;

pub(crate) fn start(commands: Sender<Command>) {
    let spawned = std::thread::Builder::new()
        .name("resume-listener".into())
        .spawn(move || {
            if let Err(error) = listen(commands) {
                crate::logger::info(format!("Linux resume listener unavailable: {error:#}"));
            }
        });
    if let Err(error) = spawned {
        crate::logger::info(format!("Linux resume listener could not start: {error}"));
    }
}

fn listen(commands: Sender<Command>) -> zbus::Result<()> {
    let connection = zbus::blocking::Connection::system()?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    for message in proxy.receive_signal("PrepareForSleep")? {
        handle_signal(&message, &commands);
    }
    Ok(())
}

fn handle_signal(message: &zbus::Message, commands: &Sender<Command>) {
    // true means about to sleep, false means resumed. Existing refresh
    // handling preserves provider backoff without forcing activation.
    if matches!(message.body().deserialize::<(bool,)>(), Ok((false,)))
        && commands.send(Command::RefreshAfterResume).is_ok()
    {
        super::wake::wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_well_formed_resume_signal_requests_refresh() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let signal = || {
            zbus::Message::signal(
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
                "PrepareForSleep",
            )
            .unwrap()
        };
        handle_signal(&signal().build(&(true,)).unwrap(), &sender);
        handle_signal(&signal().build(&("invalid",)).unwrap(), &sender);
        assert!(receiver.try_recv().is_err());
        handle_signal(&signal().build(&(false,)).unwrap(), &sender);
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::RefreshAfterResume)
        ));
        assert!(receiver.try_recv().is_err());
    }
}
