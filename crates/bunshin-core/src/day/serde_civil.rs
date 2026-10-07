//! Civil JSON strings without enabling jiff's serde dependency feature.
pub(super) mod date {
    use jiff::civil::Date;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        date: impl std::borrow::Borrow<Date>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(date.borrow())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Date, D::Error> {
        let value = String::deserialize(deserializer)?;
        value
            .parse()
            .map_err(|_| serde::de::Error::custom("invalid civil date"))
    }
}
pub(super) mod optional_time {
    use jiff::civil::Time;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        time: impl std::borrow::Borrow<Option<Time>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match time.borrow() {
            Some(time) => {
                serializer.serialize_some(&format!("{:02}:{:02}", time.hour(), time.minute()))
            }
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Time>, D::Error> {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|value| {
                if value.len() != 5
                    || value.as_bytes()[2] != b':'
                    || !value
                        .bytes()
                        .enumerate()
                        .all(|(index, byte)| index == 2 || byte.is_ascii_digit())
                {
                    return Err(serde::de::Error::custom("invalid HH:MM time"));
                }
                value
                    .parse()
                    .map_err(|_| serde::de::Error::custom("invalid HH:MM time"))
            })
            .transpose()
    }
}
