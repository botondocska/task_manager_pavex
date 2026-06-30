use pavex::{Blueprint, blueprint::from};
pub mod users;

pub fn router(bp: &mut Blueprint) {
    bp.routes(from![crate::routes::users]);
}