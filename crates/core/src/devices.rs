//! The owner's machines, and whether each one answers.

use crate::model::*;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const COLUMNS: &str = "id, name, kind, address, port, note";

fn row_to_device(r: &Row) -> rusqlite::Result<Device> {
    Ok(Device {
        id: r.get(0)?,
        name: r.get(1)?,
        kind: DeviceKind::parse(&r.get::<_, String>(2)?),
        address: r.get(3)?,
        port: r.get::<_, i64>(4)? as u16,
        note: r.get(5)?,
    })
}

fn required(field: &str, value: &str) -> Result<String> {
    let v = value.trim();
    if v.is_empty() { Err(Error::Invalid(format!("A device needs {field}."))) } else { Ok(v.to_string()) }
}

impl Db {
    pub fn create_device(&mut self, d: NewDevice) -> Result<Device> {
        let name = required("a name", &d.name)?;
        let address = required("an address", &d.address)?;
        self.conn.execute(
            "INSERT INTO devices (name, kind, address, port, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, d.kind.as_str(), address, d.port.unwrap_or(22), d.note.unwrap_or_default()],
        )?;
        self.get_device(self.conn.last_insert_rowid())
    }

    pub fn get_device(&self, id: i64) -> Result<Device> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM devices WHERE id = ?1"), [id], row_to_device)
            .optional()?
            .ok_or(Error::NotFound("device", id))
    }

    pub fn list_devices(&self) -> Result<Vec<Device>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM devices ORDER BY name COLLATE NOCASE"))?;
        Ok(stmt.query_map([], row_to_device)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn update_device(&mut self, id: i64, p: DevicePatch) -> Result<Device> {
        let mut d = self.get_device(id)?;
        if let Some(n) = p.name {
            d.name = required("a name", &n)?;
        }
        if let Some(k) = p.kind {
            d.kind = k;
        }
        if let Some(a) = p.address {
            d.address = required("an address", &a)?;
        }
        if let Some(port) = p.port {
            d.port = port;
        }
        if let Some(note) = p.note {
            d.note = note;
        }
        self.conn.execute(
            "UPDATE devices SET name = ?2, kind = ?3, address = ?4, port = ?5, note = ?6 WHERE id = ?1",
            params![id, d.name, d.kind.as_str(), d.address, d.port, d.note],
        )?;
        self.get_device(id)
    }

    pub fn delete_device(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM devices WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("device", id));
        }
        Ok(())
    }
}

/// Whether something accepts a TCP connection at this address and port within the timeout.
pub fn probe(address: &str, port: u16, timeout: Duration) -> bool {
    let Ok(addrs) = (address, port).to_socket_addrs() else { return false };
    addrs.into_iter().any(|a| TcpStream::connect_timeout(&a, timeout).is_ok())
}
