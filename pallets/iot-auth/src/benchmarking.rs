#![cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use crate::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
  // Add individual benchmarks here
  benchmark_name {
     /* code to set the initial state */
  }: {
     /* code to test the function benchmarked */
  }
  verify {
     /* optional verification */
  }
}