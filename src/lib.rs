//! 服务器与客户端验收设施的共享入口。
mod auth;
mod compressed_assets;
mod compressed_http;
pub mod connection;
pub mod cpu;
mod mock;
pub mod netem;
mod ratelimit;
mod response;
mod rest;
pub mod routes;
pub mod state;
mod stats;
mod stomp;
mod testkit;
mod ws;
