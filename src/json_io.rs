use serde::{Serialize, de::DeserializeOwned};
use std::io::Write;

pub fn read<T: DeserializeOwned>(input: &[u8]) -> serde_json::Result<T> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    deserializer.disable_recursion_limit();
    let result = T::deserialize(serde_stacker::Deserializer::new(&mut deserializer))?;
    deserializer.end()?;
    Ok(result)
}

pub fn write<T: Serialize>(value: &T, mut writer: impl Write) -> crate::error::Result<()> {
    let mut serializer = serde_json::Serializer::pretty(&mut writer);
    value.serialize(serde_stacker::Serializer::new(&mut serializer))?;
    writeln!(writer)?;
    writer.flush()?;
    Ok(())
}
