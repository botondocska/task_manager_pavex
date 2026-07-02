use pavex::{Blueprint, blueprint::from};
pub mod pages;
pub mod users;

pub fn router(bp: &mut Blueprint) {
    bp.routes(from![crate::routes::pages]);
}

pub fn api_router(bp: &mut Blueprint) {
    bp.routes(from![crate::routes::users]);
}
