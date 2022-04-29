use std::option::Option;
use serde::Deserialize;
use super::super::utils;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RequestReviewConfigDto {
  pub verify_isolation: Option<bool>
}

impl utils::Mergable for RequestReviewConfigDto {
  /// Merge the provided b with self overriding with any present values
  fn merge(&self, b: &Self) -> Self {
    RequestReviewConfigDto {
      verify_isolation: b.verify_isolation.or(self.verify_isolation)
    }
  }
}
