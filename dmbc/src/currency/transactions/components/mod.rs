//! Common transaction components.

mod fees;
mod intermediary;

pub use currency::transactions::components::fees::{FeeStrategy, ThirdPartyFees};
pub use currency::transactions::components::intermediary::Intermediary;
