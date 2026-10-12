//! P6-H is test-only. Every authoritative operation uses the real mounted runtime.
use super::super::*;
#[path = "runtime_host_gateway_p6_crash_tests.rs"]
mod crash;
#[path = "runtime_host_gateway_p6_fixture_tests.rs"]
mod fixture;
#[path = "runtime_host_gateway_p6_host_tests.rs"]
mod host;
#[path = "runtime_host_gateway_p6_load_tests.rs"]
mod load;
#[path = "runtime_host_gateway_p6_topology_tests.rs"]
mod topology;
