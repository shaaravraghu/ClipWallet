pub mod disk;
pub mod encrypted;
pub mod ram;
pub mod spill;

pub use ram::RamStore;
#[allow(unused_imports)]
pub use encrypted::{
    delete_from_vault, get_or_create_key,
    list_vault_ids,    load_from_vault,
    save_to_vault,     delete_key,
};