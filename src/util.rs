use rand::{thread_rng, Rng, distributions::Alphanumeric};

const UNIQUE_ID_LENGTH: usize = 16;

/// Generates a unique ID that matches the database datatype (varchar(16))
pub(crate) fn gen_unique_id() -> String {
    let mut rng = thread_rng();
    (0..UNIQUE_ID_LENGTH).map(|_| rng.sample(Alphanumeric) as char).collect()
}