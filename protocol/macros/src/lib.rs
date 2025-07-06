extern crate proc_macro;
use std::sync::atomic::AtomicI16;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

static LAST_PACKET_ID: AtomicI16 = AtomicI16::new(-1);

#[proc_macro_derive(Packet)]
pub fn derive_packet_id(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let new_id = LAST_PACKET_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    format!("impl Packet for {name} {{ const ID: u8 = {new_id}; }}")
        .parse()
        .unwrap()
}
