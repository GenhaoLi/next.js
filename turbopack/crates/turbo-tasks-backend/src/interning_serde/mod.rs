//! Exposed for usage in `turbo-tasks-backend`

use std::io::Write;

use indexmap::IndexSet;
use rustc_hash::FxBuildHasher;
use serde::{de::DeserializeOwned, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::FxIndexSet;

pub fn to_vec<T>(config: &pot::Config, value: &T) -> anyhow::Result<(Vec<u8>, RcStrToLocalId)>
where
    T: Serialize,
{
    let mut vec = Vec::new();
    let ser_map = to_writer(config, value, &mut vec)?;
    Ok((vec, ser_map))
}

#[derive(Default)]
pub struct RcStrToLocalId(IndexSet<RcStr, FxBuildHasher>);

#[derive(Default)]
pub struct LocalIdToGlobalId(Vec<u32>);

impl From<Vec<u32>> for LocalIdToGlobalId {
    fn from(value: Vec<u32>) -> Self {
        Self(value)
    }
}

impl LocalIdToGlobalId {
    pub fn write_to(&self, writer: &mut impl Write) -> anyhow::Result<()> {
        let len = self.0.len() as u32;
        for id in self.0.iter().rev() {
            writer.write_all(&id.to_le_bytes())?;
        }

        writer.write_all(&len.to_le_bytes())?;

        Ok(())
    }

    fn read_from_slice(mut bytes: &[u8]) -> anyhow::Result<(Self, &[u8])> {
        let mut global_ids = Vec::new();

        // Length is the last 4 bytes
        let len = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
        global_ids.reserve(len as usize);

        bytes = &bytes[..bytes.len() - 4];

        // Read the ids in reverse order
        for _ in 0..len {
            let id = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
            global_ids.push(id);
            bytes = &bytes[..bytes.len() - 4];
        }

        Ok((Self(global_ids), bytes))
    }
}

pub fn to_writer<T, W>(config: &pot::Config, value: &T, writer: W) -> anyhow::Result<RcStrToLocalId>
where
    T: Serialize,
    W: Write,
{
    let (result, ser_map) = turbo_rcstr::set_ser_map(|| config.serialize_into(value, writer));
    result?;

    Ok(RcStrToLocalId(ser_map))
}

pub fn from_slice<T>(
    config: &pot::Config,
    bytes: &[u8],
    query_db: impl FnMut(u32) -> anyhow::Result<RcStr>,
) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let (global_ids, bytes) = LocalIdToGlobalId::read_from_slice(bytes)?;

    let de_map = global_ids
        .0
        .into_iter()
        .map(query_db)
        .collect::<anyhow::Result<Vec<_>>>()?;

    turbo_rcstr::set_de_map(&de_map, || Ok(config.deserialize(bytes)?))
}
