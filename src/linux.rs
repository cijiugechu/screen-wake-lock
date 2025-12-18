use crate::{WakeLockError, WakeLockResult};
use std::collections::BTreeMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};

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
        conn: Connection,
        fd: OwnedFd,
    },
}

pub struct PlatformWakeLock {
    backend: Backend,
}

impl PlatformWakeLock {
    pub fn acquire(reason: &str) -> WakeLockResult<Self> {
        // Prefer session-bus mechanisms when available.
        if let Ok(conn) = Connection::session() {
            if let Ok(cookie) = try_gnome_session(&conn, reason) {
                return Ok(Self {
                    backend: Backend::GnomeSession { conn, cookie },
                });
            }
            if let Ok(cookie) = try_fdo_screensaver(&conn, reason) {
                return Ok(Self {
                    backend: Backend::FdoScreenSaver { conn, cookie },
                });
            }
            if let Ok(cookie) = try_fdo_powermanagement(&conn, reason) {
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
        if let Ok((conn, fd)) = try_logind(reason) {
            return Ok(Self {
                backend: Backend::Logind { conn, fd },
            });
        }

        Err(WakeLockError::Platform(
            "no suitable Linux inhibition backend found".to_string(),
        ))
    }

    pub fn release(self) -> WakeLockResult<()> {
        match self.backend {
            Backend::GnomeSession { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.gnome.SessionManager",
                    "/org/gnome/SessionManager",
                    "org.gnome.SessionManager",
                )?;
                let _ = proxy.call::<(), _>("Uninhibit", &(cookie));
                Ok(())
            }
            Backend::FdoScreenSaver { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.ScreenSaver",
                    "/org/freedesktop/ScreenSaver",
                    "org.freedesktop.ScreenSaver",
                )?;
                let _ = proxy.call::<(), _>("UnInhibit", &(cookie));
                Ok(())
            }
            Backend::FdoPowerManagement { conn, cookie } => {
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.PowerManagement",
                    "/org/freedesktop/PowerManagement/Inhibit",
                    "org.freedesktop.PowerManagement.Inhibit",
                )?;
                let _ = proxy.call::<(), _>("UnInhibit", &(cookie));
                Ok(())
            }
            Backend::XdgPortal { conn, handle } => {
                // Release by calling Request.Close on the returned handle.
                let proxy = Proxy::new(
                    &conn,
                    "org.freedesktop.portal.Desktop",
                    handle,
                    "org.freedesktop.portal.Request",
                )?;
                let _ = proxy.call::<(), _>("Close", &());
                Ok(())
            }
            Backend::Logind { conn: _, fd: _ } => {
                // The inhibitor is released when the FD is closed (dropped).
                Ok(())
            }
        }
    }
}

fn try_gnome_session(conn: &Connection, reason: &str) -> WakeLockResult<u32> {
    let proxy = Proxy::new(
        conn,
        "org.gnome.SessionManager",
        "/org/gnome/SessionManager",
        "org.gnome.SessionManager",
    )?;
    let cookie: u32 = proxy.call("Inhibit", &("wake_lock", 0u32, reason, GNOME_INHIBIT_IDLE))?;
    Ok(cookie)
}

fn try_fdo_screensaver(conn: &Connection, reason: &str) -> WakeLockResult<u32> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        "org.freedesktop.ScreenSaver",
    )?;
    let cookie: u32 = proxy.call("Inhibit", &("wake_lock", reason))?;
    Ok(cookie)
}

fn try_fdo_powermanagement(conn: &Connection, reason: &str) -> WakeLockResult<u32> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.PowerManagement",
        "/org/freedesktop/PowerManagement/Inhibit",
        "org.freedesktop.PowerManagement.Inhibit",
    )?;
    let cookie: u32 = proxy.call("Inhibit", &("wake_lock", reason))?;
    Ok(cookie)
}

fn try_xdg_portal(conn: &Connection, reason: &str) -> WakeLockResult<OwnedObjectPath> {
    let proxy = Proxy::new(
        conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Inhibit",
    )?;

    let mut options: BTreeMap<String, OwnedValue> = BTreeMap::new();
    options.insert("reason".to_string(), OwnedValue::from(reason.to_string()));

    // flags: 8 = Idle
    let handle: OwnedObjectPath = proxy.call("Inhibit", &("", PORTAL_INHIBIT_IDLE, options))?;
    Ok(handle)
}

fn try_logind(reason: &str) -> WakeLockResult<(Connection, OwnedFd)> {
    let conn = Connection::system()?;
    let proxy = Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;

    // what: "idle" (inhibit idle actions), mode: "block".
    let fd: OwnedFd = proxy.call("Inhibit", &("idle", "wake_lock", reason, "block"))?;
    Ok((conn, fd))
}
