use rand::{distributions::Alphanumeric, thread_rng, Rng};

#[cfg(feature = "yaml")]
pub(crate) mod yaml {
    use std::fmt::Display;

    use anyhow::{anyhow, Result};

    pub(crate) trait YamlExt {
        fn field_as_str<I: serde_yaml::Index + Display>(&self, index: I) -> Result<&str>;
        fn field_as_vec_str<I: serde_yaml::Index + Display>(&self, index: I) -> Result<Vec<&str>>;
        fn field_as_sequence<I: serde_yaml::Index + Display>(
            &self,
            index: I,
        ) -> Result<&Vec<serde_yaml::Value>>;
    }

    impl YamlExt for serde_yaml::Value {
        fn field_as_str<I: serde_yaml::Index + Display>(&self, index: I) -> Result<&str> {
            self.get(&index)
                .ok_or_else(|| anyhow!("Missing field {index}"))?
                .as_str()
                .ok_or_else(|| anyhow!("Field {index} is not a str"))
        }

        fn field_as_vec_str<I: serde_yaml::Index + Display>(&self, index: I) -> Result<Vec<&str>> {
            self.get(&index)
                .ok_or_else(|| anyhow!("Missing field {index}"))?
                .as_sequence()
                .ok_or_else(|| anyhow!("Field {index} is not a sequence"))?
                .into_iter()
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| anyhow!("Entry in sequence is not a str"))
                })
                .collect()
        }

        fn field_as_sequence<I: serde_yaml::Index + Display>(
            &self,
            index: I,
        ) -> Result<&Vec<serde_yaml::Value>> {
            self.get(&index)
                .ok_or_else(|| anyhow!("Missing field {index}"))?
                .as_sequence()
                .ok_or_else(|| anyhow!("Field {index} is not a sequence"))
        }
    }
}

const UNIQUE_ID_LENGTH: usize = 16;

/// Generates a unique ID that matches the database datatype (varchar(16))
pub(crate) fn gen_unique_id() -> String {
    let mut rng = thread_rng();
    (0..UNIQUE_ID_LENGTH)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}
