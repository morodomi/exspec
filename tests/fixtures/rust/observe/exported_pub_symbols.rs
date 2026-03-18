pub fn create_user() {}

pub struct User {
    pub name: String,
}

pub enum Status {
    Active,
    Inactive,
}

pub type UserId = u64;

pub const MAX_USERS: usize = 100;

pub static GLOBAL_FLAG: bool = true;

pub trait Authenticatable {
    fn authenticate(&self) -> bool;
}

fn private_helper() {}
struct InternalState;
