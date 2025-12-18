use crate::{Error, LinuxOptions};
use std::collections::BTreeMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Str};

const GNOME_INHIBIT_IDLE: u32 = 8;
const PORTAL_INHIBIT_IDLE: u32 = 8;

enum Backend {
    // Session bus APIs (cookie + same connection must remain alive)
    GnomeSession {
        conn: Connection,
        cookie: u32,
    },
    FdoScreenSaver {
        conn: Connection,
        cookie: u32,
    },
    FdoPowerManagement {
        conn: Connection,
        cookie: u32,
    },
    XdgPortal {
        conn: Connection,
        handle: OwnedObjectPath,
    },

    // System bus (fd must remain open)
    Logind {
        _conn: Connection,
        _fd: OwnedFd,
    },
}

pub struct PlatformWakeLock {
    backend: Backend,
}

impl PlatformWakeLock {
    pub fn acquire(application_id: &str, reason: &str) -> Result<Self, Error> {
        // Prefer session-bus mechanisms when available.
        if let Ok(conn) = Connection::session() {
            if let Ok(cookie) = try_gnome_session(&conn, application_id, reason) {
                return Ok(Self {
                    backend: Backend::GnomeSession { conn, cookie },
                });
            }
            if let Ok(cookie) = try_fdo_screensaver(&conn, application_id, reason) {
                return Ok(Self {
                    backend: Backend::FdoScreenSaver { conn, cookie },
                });
            }
            if let Ok(cookie) = try_fdo_powermanagement(&conn, application_id, reason) {
                return Ok(Self {
                    backend: Backend::FdoPowerManagement { conn, cookie },
                });
            }
            if let Ok(handle) = try_xdg_portal(&conn, reason) {
                return Ok(Self {
                    backend: Backend::XdgPortal { conn, handle },
                });
            }
        }

        // Fallback: systemd-logind idle inhibitor (system bus).
        if let Ok((conn, fd)) = try_logind(application_id, reason) {
            return Ok(Self {
                backend: Backend::Logind {
                    _conn: conn,
                    _fd: fd,
                },
            });
        }

        Err(Error::Unsupported(
            "no suitable Linux inhibition backend found".to_string(),
        ))
    }

    pub fn release(self) -> Result<(), Error> {
        match self.backend {
            Backend::GnomeSession { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.gnome.SessionManager",
                    "/org/gnome/SessionManager",
                    "org.gnome.SessionManager",
                )
                .map_err(|e| Error::Dbus(e.to_string()))?;
                let _: zbus::Result<()> = proxy.call("Uninhibit", &(cookie));
                Ok(())
            }
            Backend::FdoScreenSaver { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.ScreenSaver",
                    "/org/freedesktop/ScreenSaver",
                    "org.freedesktop.ScreenSaver",
                )
                .map_err(|e| Error::Dbus(e.to_string()))?;
                let _: zbus::Result<()> = proxy.call("UnInhibit", &(cookie));
                Ok(())
            }
            Backend::FdoPowerManagement { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.PowerManagement",
                    "/org/freedesktop/PowerManagement/Inhibit",
                    "org.freedesktop.PowerManagement.Inhibit",
                )
                .map_err(|e| Error::Dbus(e.to_string()))?;
                let _: zbus::Result<()> = proxy.call("UnInhibit", &(cookie));
                Ok(())
            }
            Backend::XdgPortal { conn, handle } => {
                // Release by calling Request.Close on the returned handle.
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.portal.Desktop",
                    handle,
                    "org.freedesktop.portal.Request",
                )
                .map_err(|e| Error::Dbus(e.to_string()))?;
                let _: zbus::Result<()> = proxy.call("Close", &());
                Ok(())
            }
            Backend::Logind { .. } => {
                // The inhibitor is released when the FD is closed (dropped).
                Ok(())
            }
        }
    }
}

pub struct Inner {
    lock: Option<PlatformWakeLock>,
    active: bool,
}

pub fn is_supported() -> bool {
    Connection::session().is_ok() || Connection::system().is_ok()
}

pub fn acquire(reason: &str, linux: LinuxOptions) -> Result<Inner, Error> {
    let application_id = linux
        .application_id
        .as_deref()
        .unwrap_or("screen_wake_lock");
    let effective_reason = linux.reason.as_deref().unwrap_or(reason);

    let lock = PlatformWakeLock::acquire(application_id, effective_reason)?;
    Ok(Inner {
        lock: Some(lock),
        active: true,
    })
}

pub fn release(inner: &mut Inner) {
    if !inner.active {
        return;
    }
    // Best-effort: ignore D-Bus errors during release.
    if let Some(lock) = inner.lock.take() {
        let _ = lock.release();
    }
    inner.active = false;
}

fn try_gnome_session(conn: &Connection, application_id: &str, reason: &str) -> zbus::Result<u32> {
    let proxy = Proxy::new(
        conn,
        "org.gnome.SessionManager",
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
    )?;
    let cookie: u32 = proxy.call(
        "Inhibit",
        &(application_id, 0u32, reason, GNOME_INHIBIT_IDLE),
    )?;
    Ok(cookie)
}

fn try_fdo_screensaver(conn: &Connection, application_id: &str, reason: &str) -> zbus::Result<u32> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        "org.freedesktop.ScreenSaver",
    )?;
    let cookie: u32 = proxy.call("Inhibit", &(application_id, reason))?;
    Ok(cookie)
}

fn try_fdo_powermanagement(
    conn: &Connection,
    application_id: &str,
    reason: &str,
) -> zbus::Result<u32> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.PowerManagement",
        "/org/freedesktop/PowerManagement/Inhibit",
        "org.freedesktop.PowerManagement.Inhibit",
    )?;
    let cookie: u32 = proxy.call("Inhibit", &(application_id, reason))?;
    Ok(cookie)
}

fn try_xdg_portal(conn: &Connection, reason: &str) -> zbus::Result<OwnedObjectPath> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Inhibit",
    )?;

    let mut options: BTreeMap<String, OwnedValue> = BTreeMap::new();
    options.insert("reason".to_string(), OwnedValue::from(Str::from(reason)));

    // flags: 8 = Idle
    let handle: OwnedObjectPath = proxy.call("Inhibit", &("", PORTAL_INHIBIT_IDLE, options))?;
    Ok(handle)
}

fn try_logind(application_id: &str, reason: &str) -> zbus::Result<(Connection, OwnedFd)> {
    let conn = Connection::system()?;
    let proxy = Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;

    // what: "idle" (inhibit idle actions), mode: "block".
    let fd: OwnedFd = proxy.call("Inhibit", &("idle", application_id, reason, "block"))?;
    Ok((conn, fd))
}
