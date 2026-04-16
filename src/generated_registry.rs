// SPDX-License-Identifier: GPL-2.0-or-later

/// A command entry parsed from hmp-commands.hx / hmp-commands-info.hx
#[derive(Debug)]
#[allow(dead_code)]
pub struct HxEntry {
    pub name: &'static str,
    pub args_type: &'static str,
    pub params: &'static str,
    pub help: &'static str,
    pub flags: &'static str,
    pub doc: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/generated_registry.rs"));
