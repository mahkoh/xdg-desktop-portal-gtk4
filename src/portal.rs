use {
    crate::gui::UiProxy,
    portals::file_chooser::FileChooser,
    std::thread,
    thiserror::Error,
    zbus::{
        blocking::{fdo::DBusProxy as DBusProxyBlocking, Connection},
        fdo::RequestNameFlags,
    },
};

mod portals;
mod request;
mod response;

const NAME: &str = "org.freedesktop.impl.portal.desktop.gtk4";
const PATH: &str = "/org/freedesktop/portal/desktop";

#[derive(Debug, Error)]
pub enum PortalError {
    #[error("Could not connect to session bus")]
    Connection(#[source] zbus::Error),
    #[error("Could not acquire name {}", NAME)]
    AcquireName(#[source] zbus::Error),
    #[error("Could not add an interface")]
    AddInterface(#[source] zbus::Error),
    #[error("Could create dbus proxy")]
    CreateDbusProxy(#[source] zbus::Error),
    #[error("Could subscribe to name-lost events")]
    SubscribeNameLost(#[source] zbus::Error),
}

pub struct Portal {
    _session: Connection,
}

impl Portal {
    pub fn create(proxy: &UiProxy, replace: bool) -> Result<Self, PortalError> {
        let session = Connection::session().map_err(PortalError::Connection)?;

        macro_rules! add {
            ($interface:expr) => {
                session
                    .object_server()
                    .at(PATH, $interface)
                    .map_err(PortalError::AddInterface)?;
            };
        }
        add!(FileChooser::new(proxy));

        let mut name_lost_iterator = DBusProxyBlocking::new(&session)
            .map_err(PortalError::CreateDbusProxy)?
            .receive_name_lost()
            .map_err(PortalError::SubscribeNameLost)?;
        thread::spawn(move || {
            name_lost_iterator.next();
            log::warn!("Lost name {}", NAME);
            std::process::exit(0);
        });

        let mut flags = RequestNameFlags::AllowReplacement | RequestNameFlags::DoNotQueue;
        if replace {
            flags |= RequestNameFlags::ReplaceExisting;
        }
        session
            .request_name_with_flags(NAME, flags)
            .map_err(PortalError::AcquireName)?;
        Ok(Self { _session: session })
    }
}
