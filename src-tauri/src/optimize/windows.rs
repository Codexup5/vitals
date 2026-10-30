//! The Windows side of the optimizer: registry values of both hives, `SPI_SETMOUSE`, the power scheme, the hardware
//! facts, the restore point and the Explorer restart.

#![allow(clippy::unnecessary_wraps, reason = "stubs until step 10")]

use super::{Data, Hive, KeyAbsent, RegWrite};

/// A value of a key from the root of a hive.
pub fn read(_hive: Hive, _key: &str, _name: &str) -> Result<Option<Data>, KeyAbsent> {
    Ok(None)
}

/// One registry write; a key is deleted only when it holds nothing but the path to the value the write names.
pub fn put(_write: &RegWrite) -> Result<(), String> {
    Err("not built yet".to_owned())
}

#[cfg(test)]
mod tests {
    use vitals_ipc::TweakId;

    use super::super::Put;
    use super::*;

    /// Deletes the scratch key even when an assertion fails.
    struct Scratch(String);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = put(&RegWrite {
                id: TweakId::GameMode,
                hive: Hive::User,
                key: self.0.clone(),
                name: String::new(),
                put: Put::DeleteKey {
                    key: self.0.clone(),
                },
            });
        }
    }

    fn write(key: &str, name: &str, put: Put) -> RegWrite {
        RegWrite {
            id: TweakId::GameMode,
            hive: Hive::User,
            key: key.to_owned(),
            name: name.to_owned(),
            put,
        }
    }

    #[test]
    fn a_dword_a_string_and_an_absent_value_round_trip_in_a_scratch_key() {
        let root = format!(r"Software\Vitals\test-{}", std::process::id());
        let key = format!(r"{root}\inner");
        // Software\Vitals may not exist yet: the first missing key is the one a write creates and a revert deletes
        let absent = read(Hive::User, &key, "Mode");
        let Err(KeyAbsent { created }) = absent else {
            panic!("{absent:?}: the scratch key exists");
        };
        assert!(
            root.to_ascii_lowercase()
                .starts_with(&created.to_ascii_lowercase()),
            "{created} is not on the path of {root}"
        );
        let scratch = Scratch(created.clone());

        put(&write(
            &key,
            "Mode",
            Put::Data {
                data: Data::Dword(2),
            },
        ))
        .unwrap();
        assert_eq!(read(Hive::User, &key, "Mode"), Ok(Some(Data::Dword(2))));
        put(&write(
            &key,
            "Speed",
            Put::Data {
                data: Data::Text("0".to_owned()),
            },
        ))
        .unwrap();
        assert_eq!(
            read(Hive::User, &key, "Speed"),
            Ok(Some(Data::Text("0".to_owned())))
        );
        put(&write(
            &key,
            "",
            Put::Data {
                data: Data::Text(String::new()),
            },
        ))
        .unwrap();
        assert_eq!(
            read(Hive::User, &key, ""),
            Ok(Some(Data::Text(String::new())))
        );
        assert_eq!(read(Hive::User, &key, "Gone"), Ok(None));

        put(&write(&key, "Mode", Put::DeleteValue)).unwrap();
        assert_eq!(read(Hive::User, &key, "Mode"), Ok(None));
        put(&write(&key, "Mode", Put::DeleteValue)).unwrap();

        // The key holds a value the change did not write: it is kept, the named value goes
        put(&write(
            &key,
            "Speed",
            Put::DeleteKey {
                key: created.clone(),
            },
        ))
        .unwrap();
        assert_eq!(read(Hive::User, &key, "Speed"), Ok(None));
        assert_eq!(
            read(Hive::User, &key, ""),
            Ok(Some(Data::Text(String::new())))
        );

        put(&write(
            &key,
            "",
            Put::DeleteKey {
                key: created.clone(),
            },
        ))
        .unwrap();
        assert!(matches!(read(Hive::User, &key, ""), Err(KeyAbsent { .. })));
        assert!(matches!(
            read(Hive::User, &created, ""),
            Err(KeyAbsent { .. })
        ));
        drop(scratch);
    }
}
