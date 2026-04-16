// SPDX-License-Identifier: GPL-2.0-or-later

mod announce_self;
mod balloon;
mod block_job_cancel;
mod block_job_complete;
mod block_job_pause;
mod block_job_resume;
mod block_job_set_speed;
mod block_resize;
mod block_set_io_throttle;
mod block_stream;
mod boot_set;
mod calc_dirty_rate;
mod cancel_vcpu_dirty_limit;
mod change;
mod chardev_add;
mod chardev_remove;
mod chardev_send_break;
mod client_migrate_info;
mod closefd;
mod commit;
mod cont;
mod cpu;
mod delvm;
mod device_add;
mod device_del;
mod drive_add;
mod drive_backup;
mod drive_del;
mod drive_mirror;
mod dump_guest_memory;
mod dump_skeys;
mod dumpdtb;
mod eject;
mod exit_preconfig;
mod expire_password;
mod gdbserver;
mod getfd;
mod gpa2hpa;
mod gpa2hva;
mod gva2gpa;
mod hostfwd_add;
mod hostfwd_remove;
mod info_accel;
mod info_accelerators;
mod info_balloon;
mod info_block;
mod info_block_jobs;
mod info_blockstats;
mod info_chardev;
mod info_cmma;
pub(crate) mod info_cpus;
mod info_cryptodev;
mod info_dirty_rate;
mod info_dump;
mod info_firmware_log;
mod info_hotpluggable_cpus;
mod info_iothreads;
mod info_irq;
mod info_jit;
mod info_kvm;
mod info_lapic;
mod info_mem;
mod info_memdev;
mod info_memory_devices;
mod info_memory_size_summary;
mod info_mice;
mod info_migrate;
mod info_migrate_capabilities;
mod info_migrate_parameters;
mod info_mtree;
mod info_name;
mod info_network;
mod info_numa;
mod info_pci;
mod info_pic;
mod info_qdm;
mod info_qom_tree;
mod info_qtree;
mod info_ramblock;
mod info_registers;
mod info_rocker;
mod info_roms;
mod info_sev;
mod info_sgx;
mod info_skeys;
mod info_snapshots;
mod info_spice;
mod info_stats;
mod info_status;
mod info_sync_profile;
mod info_tlb;
mod info_tpm;
mod info_trace_events;
mod info_usb;
mod info_usbhost;
mod info_usernet;
mod info_uuid;
mod info_vcpu_dirty_limit;
mod info_version;
mod info_via;
mod info_virtio;
mod info_vm_generation_id;
mod info_vnc;
mod ioport;
mod loadvm;
mod log;
mod logfile;
mod mce;
mod memory_dump;
mod memsave;
mod migrate;
mod migrate_cancel;
mod migrate_continue;
mod migrate_incoming;
mod migrate_pause;
mod migrate_recover;
mod migrate_set_capability;
mod migrate_set_parameter;
mod migrate_start_postcopy;
mod migration_mode;
mod mouse_button;
mod mouse_move;
mod mouse_set;
mod nbd_server;
mod netdev_add;
mod netdev_del;
mod nmi;
mod object_add;
mod object_del;
mod one_insn_per_tb;
mod pcie_aer_inject_error;
mod print;
mod qom_get;
mod qom_list;
mod qom_set;
mod quit;
mod replay;
mod ringbuf;
mod savevm;
mod screendump;
mod sendkey;
mod set_link;
mod set_password;
mod set_vcpu_dirty_limit;
mod snapshot_blkdev;
mod snapshot_blkdev_internal;
mod snapshot_delete_blkdev_internal;
mod stop;
mod sum;
mod sync_profile;
mod system_powerdown;
mod system_reset;
mod system_wakeup;
mod trace_event;
mod trace_file;
mod watchdog_action;
mod x_colo_lost_heartbeat;
mod xen_event;

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;

use qapi::ExecuteError;

use crate::args::{parse_arg_defs, parse_args, ArgValue};
use crate::expr::eval_expr;
use crate::generated_registry::{HxEntry, HMP_COMMANDS, HMP_INFO_COMMANDS};
use crate::qmp::QmpConnection;

/// Structured error type for command handlers.
///
/// Preserves the distinction between a lost QMP connection and a
/// recoverable command-level error, so callers can react without
/// string-matching on error text.
#[derive(Debug)]
pub enum CmdError {
    /// The QMP connection was lost (broken pipe, connection reset, EOF).
    Disconnected,
    /// A QMP-level or argument-level error that doesn't indicate
    /// a lost connection.
    Command(String),
}

impl From<ExecuteError> for CmdError {
    fn from(e: ExecuteError) -> Self {
        if let ExecuteError::Io(ref io_err) = e {
            if matches!(
                io_err.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::UnexpectedEof
            ) {
                return CmdError::Disconnected;
            }
        }
        CmdError::Command(e.to_string())
    }
}

/// The outcome of dispatching a command line.
pub enum DispatchOutput {
    /// Human-readable output to display (may be empty).
    Output(String),
    /// The QMP connection was lost.
    Disconnected,
}

/// The type of an async handler function for an HMP command.
///
/// Handlers receive a reference to the QMP connection and a map of parsed
/// argument values.  They return either a human-readable output string
/// or a [`CmdError`].
pub type HandlerFn =
    for<'a> fn(
        &'a QmpConnection,
        &'a HashMap<String, ArgValue>,
    ) -> Pin<Box<dyn Future<Output = Result<String, CmdError>> + Send + 'a>>;

/// Extract a required string argument, or return a [`CmdError`].
pub(crate) fn require_str(
    args: &HashMap<String, ArgValue>,
    name: &str,
) -> Result<String, CmdError> {
    match args.get(name) {
        Some(ArgValue::Str(s)) => Ok(s.clone()),
        _ => Err(CmdError::Command(format!(
            "missing required argument '{name}'"
        ))),
    }
}

/// Extract a required integer argument, or return a [`CmdError`].
pub(crate) fn require_int(args: &HashMap<String, ArgValue>, name: &str) -> Result<i64, CmdError> {
    match args.get(name) {
        Some(ArgValue::Int(n)) => Ok(*n),
        _ => Err(CmdError::Command(format!(
            "missing required argument '{name}'"
        ))),
    }
}

/// Extract a required boolean argument, or return a [`CmdError`].
pub(crate) fn require_bool(args: &HashMap<String, ArgValue>, name: &str) -> Result<bool, CmdError> {
    match args.get(name) {
        Some(ArgValue::Bool(b)) => Ok(*b),
        _ => Err(CmdError::Command(format!(
            "missing required argument '{name}'"
        ))),
    }
}

/// Extract and evaluate a required Long expression argument.
///
/// Long (`l`) arguments are stored as raw strings and evaluated using the
/// expression parser, which supports arithmetic, `$register` references,
/// character literals, etc.
pub(crate) async fn require_expr(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
    name: &str,
) -> Result<i64, CmdError> {
    match args.get(name) {
        Some(ArgValue::Str(s)) => eval_expr(s, conn).await,
        Some(ArgValue::Int(n)) => Ok(*n),
        _ => Err(CmdError::Command(format!(
            "missing required argument '{name}'"
        ))),
    }
}

/// Extract an optional boolean argument, defaulting to `false`.
pub(crate) fn opt_bool(args: &HashMap<String, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

/// Register a handler, wrapping the async function into the [`HandlerFn`]
/// function-pointer type.
macro_rules! register {
    ($self:ident, main, $name:expr, $handler:path) => {
        $self.set_main_handler($name, |c, a| Box::pin($handler(c, a)));
    };
    ($self:ident, info, $name:expr, $handler:path) => {
        $self.set_info_handler($name, |c, a| Box::pin($handler(c, a)));
    };
}

/// Pairs a static HMP command definition with an optional handler function.
pub struct CommandEntry {
    pub entry: &'static HxEntry,
    pub handler: Option<HandlerFn>,
}

/// Command dispatch registry.
///
/// Holds two lookup tables -- one for top-level commands and one for `info`
/// subcommands.  Populated from the generated constant arrays and then
/// augmented with Rust handler functions for the commands we support.
pub struct Registry {
    main_commands: HashMap<String, CommandEntry>,
    info_commands: HashMap<String, CommandEntry>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Build a new registry from the generated command tables.
    ///
    /// Command names that contain `|` are treated as aliases: each
    /// alternative name gets its own entry pointing to the same
    /// `HxEntry`.
    pub fn new() -> Self {
        let mut main_commands = HashMap::new();
        for entry in HMP_COMMANDS.iter() {
            for alias in entry.name.split('|') {
                let alias = alias.trim();
                if !alias.is_empty() {
                    main_commands.insert(
                        alias.to_string(),
                        CommandEntry {
                            entry,
                            handler: None,
                        },
                    );
                }
            }
        }

        let mut info_commands = HashMap::new();
        for entry in HMP_INFO_COMMANDS.iter() {
            for alias in entry.name.split('|') {
                let alias = alias.trim();
                if !alias.is_empty() {
                    info_commands.insert(
                        alias.to_string(),
                        CommandEntry {
                            entry,
                            handler: None,
                        },
                    );
                }
            }
        }

        let mut registry = Self {
            main_commands,
            info_commands,
        };
        registry.register_handlers();
        registry
    }

    /// Register all implemented command handlers.
    fn register_handlers(&mut self) {
        register!(
            self,
            main,
            "announce_self",
            announce_self::cmd_announce_self
        );
        register!(self, main, "balloon", balloon::cmd_balloon);
        register!(self, main, "boot_set", boot_set::cmd_boot_set);
        register!(
            self,
            main,
            "block_job_cancel",
            block_job_cancel::cmd_block_job_cancel
        );
        register!(
            self,
            main,
            "block_job_complete",
            block_job_complete::cmd_block_job_complete
        );
        register!(
            self,
            main,
            "block_job_pause",
            block_job_pause::cmd_block_job_pause
        );
        register!(
            self,
            main,
            "block_job_resume",
            block_job_resume::cmd_block_job_resume
        );
        register!(
            self,
            main,
            "block_job_set_speed",
            block_job_set_speed::cmd_block_job_set_speed
        );
        register!(self, main, "block_resize", block_resize::cmd_block_resize);
        register!(
            self,
            main,
            "block_set_io_throttle",
            block_set_io_throttle::cmd_block_set_io_throttle
        );
        register!(self, main, "block_stream", block_stream::cmd_block_stream);
        register!(
            self,
            main,
            "calc_dirty_rate",
            calc_dirty_rate::cmd_calc_dirty_rate
        );
        register!(
            self,
            main,
            "cancel_vcpu_dirty_limit",
            cancel_vcpu_dirty_limit::cmd_cancel_vcpu_dirty_limit
        );
        register!(self, main, "c", cont::cmd_cont);
        register!(self, main, "chardev-add", chardev_add::cmd_chardev_add);
        register!(
            self,
            main,
            "chardev-change",
            chardev_add::cmd_chardev_change
        );
        register!(
            self,
            main,
            "chardev-remove",
            chardev_remove::cmd_chardev_remove
        );
        register!(self, main, "change", change::cmd_change);
        register!(
            self,
            main,
            "client_migrate_info",
            client_migrate_info::cmd_client_migrate_info
        );
        register!(self, main, "closefd", closefd::cmd_closefd);
        register!(self, main, "commit", commit::cmd_commit);
        register!(
            self,
            main,
            "chardev-send-break",
            chardev_send_break::cmd_chardev_send_break
        );
        register!(self, main, "cont", cont::cmd_cont);
        register!(self, main, "cpu", cpu::cmd_cpu);
        register!(self, main, "delvm", delvm::cmd_delvm);
        register!(self, main, "device_add", device_add::cmd_device_add);
        register!(self, main, "dumpdtb", dumpdtb::cmd_dumpdtb);
        register!(
            self,
            main,
            "dump-guest-memory",
            dump_guest_memory::cmd_dump_guest_memory
        );
        register!(self, main, "device_del", device_del::cmd_device_del);
        register!(self, main, "drive_backup", drive_backup::cmd_drive_backup);
        register!(self, main, "drive_add", drive_add::cmd_drive_add);
        register!(self, main, "drive_del", drive_del::cmd_drive_del);
        register!(self, main, "drive_mirror", drive_mirror::cmd_drive_mirror);
        register!(self, main, "dump-skeys", dump_skeys::cmd_dump_skeys);
        register!(self, main, "eject", eject::cmd_eject);
        register!(
            self,
            main,
            "exit_preconfig",
            exit_preconfig::cmd_exit_preconfig
        );
        register!(
            self,
            main,
            "expire_password",
            expire_password::cmd_expire_password
        );
        register!(self, main, "gdbserver", gdbserver::cmd_gdbserver);
        register!(self, main, "getfd", getfd::cmd_getfd);
        register!(self, main, "i", ioport::cmd_ioport_read);
        register!(self, main, "gpa2hva", gpa2hva::cmd_gpa2hva);
        register!(self, main, "gpa2hpa", gpa2hpa::cmd_gpa2hpa);
        register!(self, main, "gva2gpa", gva2gpa::cmd_gva2gpa);
        register!(self, main, "hostfwd_add", hostfwd_add::cmd_hostfwd_add);
        register!(
            self,
            main,
            "hostfwd_remove",
            hostfwd_remove::cmd_hostfwd_remove
        );
        register!(self, main, "loadvm", loadvm::cmd_loadvm);
        register!(self, main, "log", log::cmd_log);
        register!(self, main, "logfile", logfile::cmd_logfile);
        register!(self, main, "mce", mce::cmd_mce);
        register!(self, main, "memsave", memsave::cmd_memsave);
        register!(self, main, "migrate", migrate::cmd_migrate);
        register!(
            self,
            main,
            "migrate_cancel",
            migrate_cancel::cmd_migrate_cancel
        );
        register!(
            self,
            main,
            "migrate_continue",
            migrate_continue::cmd_migrate_continue
        );
        register!(
            self,
            main,
            "migrate_incoming",
            migrate_incoming::cmd_migrate_incoming
        );
        register!(
            self,
            main,
            "migrate_pause",
            migrate_pause::cmd_migrate_pause
        );
        register!(
            self,
            main,
            "migrate_recover",
            migrate_recover::cmd_migrate_recover
        );
        register!(
            self,
            main,
            "migrate_set_capability",
            migrate_set_capability::cmd_migrate_set_capability
        );
        register!(
            self,
            main,
            "migrate_set_parameter",
            migrate_set_parameter::cmd_migrate_set_parameter
        );
        register!(
            self,
            main,
            "migrate_start_postcopy",
            migrate_start_postcopy::cmd_migrate_start_postcopy
        );
        register!(
            self,
            main,
            "migration_mode",
            migration_mode::cmd_migration_mode
        );
        register!(self, main, "mouse_button", mouse_button::cmd_mouse_button);
        register!(self, main, "mouse_move", mouse_move::cmd_mouse_move);
        register!(self, main, "mouse_set", mouse_set::cmd_mouse_set);
        register!(
            self,
            main,
            "nbd_server_start",
            nbd_server::cmd_nbd_server_start
        );
        register!(self, main, "nbd_server_add", nbd_server::cmd_nbd_server_add);
        register!(
            self,
            main,
            "nbd_server_remove",
            nbd_server::cmd_nbd_server_remove
        );
        register!(
            self,
            main,
            "nbd_server_stop",
            nbd_server::cmd_nbd_server_stop
        );
        register!(self, main, "netdev_add", netdev_add::cmd_netdev_add);
        register!(self, main, "netdev_del", netdev_del::cmd_netdev_del);
        register!(self, main, "nmi", nmi::cmd_nmi);
        register!(self, main, "o", ioport::cmd_ioport_write);
        register!(self, main, "object_add", object_add::cmd_object_add);
        register!(self, main, "object_del", object_del::cmd_object_del);
        register!(
            self,
            main,
            "pcie_aer_inject_error",
            pcie_aer_inject_error::cmd_pcie_aer_inject_error
        );
        register!(self, main, "p", print::cmd_print);
        register!(self, main, "pmemsave", memsave::cmd_pmemsave);
        register!(self, main, "print", print::cmd_print);
        register!(
            self,
            main,
            "one-insn-per-tb",
            one_insn_per_tb::cmd_one_insn_per_tb
        );
        register!(self, main, "replay_break", replay::cmd_replay_break);
        register!(
            self,
            main,
            "replay_delete_break",
            replay::cmd_replay_delete_break
        );
        register!(self, main, "replay_seek", replay::cmd_replay_seek);
        register!(self, main, "ringbuf_read", ringbuf::cmd_ringbuf_read);
        register!(self, main, "ringbuf_write", ringbuf::cmd_ringbuf_write);
        register!(self, main, "q", quit::cmd_quit);
        register!(self, main, "qom-get", qom_get::cmd_qom_get);
        register!(self, main, "qom-list", qom_list::cmd_qom_list);
        register!(self, main, "qom-set", qom_set::cmd_qom_set);
        register!(self, main, "quit", quit::cmd_quit);
        register!(self, main, "s", stop::cmd_stop);
        register!(self, main, "savevm", savevm::cmd_savevm);
        register!(self, main, "screendump", screendump::cmd_screendump);
        register!(self, main, "sendkey", sendkey::cmd_sendkey);
        register!(self, main, "set_link", set_link::cmd_set_link);
        register!(self, main, "set_password", set_password::cmd_set_password);
        register!(
            self,
            main,
            "set_vcpu_dirty_limit",
            set_vcpu_dirty_limit::cmd_set_vcpu_dirty_limit
        );
        register!(
            self,
            main,
            "snapshot_blkdev",
            snapshot_blkdev::cmd_snapshot_blkdev
        );
        register!(
            self,
            main,
            "snapshot_blkdev_internal",
            snapshot_blkdev_internal::cmd_snapshot_blkdev_internal
        );
        register!(
            self,
            main,
            "snapshot_delete_blkdev_internal",
            snapshot_delete_blkdev_internal::cmd_snapshot_delete_blkdev_internal
        );
        register!(self, main, "stop", stop::cmd_stop);
        register!(self, main, "sum", sum::cmd_sum);
        register!(self, main, "sync-profile", sync_profile::cmd_sync_profile);
        register!(self, main, "trace-event", trace_event::cmd_trace_event);
        register!(self, main, "trace-file", trace_file::cmd_trace_file);
        register!(
            self,
            main,
            "system_powerdown",
            system_powerdown::cmd_system_powerdown
        );
        register!(self, main, "system_reset", system_reset::cmd_system_reset);
        register!(
            self,
            main,
            "system_wakeup",
            system_wakeup::cmd_system_wakeup
        );
        register!(
            self,
            main,
            "watchdog_action",
            watchdog_action::cmd_watchdog_action
        );
        register!(
            self,
            main,
            "x_colo_lost_heartbeat",
            x_colo_lost_heartbeat::cmd_x_colo_lost_heartbeat
        );
        register!(self, main, "x", memory_dump::cmd_x);
        register!(
            self,
            main,
            "xen-event-inject",
            xen_event::cmd_xen_event_inject
        );
        register!(self, main, "xen-event-list", xen_event::cmd_xen_event_list);
        register!(self, main, "xp", memory_dump::cmd_xp);

        register!(self, info, "accel", info_accel::cmd_info_accel);
        register!(
            self,
            info,
            "accelerators",
            info_accelerators::cmd_info_accelerators
        );
        register!(self, info, "balloon", info_balloon::cmd_info_balloon);
        register!(self, info, "block", info_block::cmd_info_block);
        register!(
            self,
            info,
            "block-jobs",
            info_block_jobs::cmd_info_block_jobs
        );
        register!(
            self,
            info,
            "blockstats",
            info_blockstats::cmd_info_blockstats
        );
        register!(self, info, "chardev", info_chardev::cmd_info_chardev);
        register!(self, info, "cmma", info_cmma::cmd_info_cmma);
        register!(self, info, "cryptodev", info_cryptodev::cmd_info_cryptodev);
        register!(
            self,
            info,
            "dirty_rate",
            info_dirty_rate::cmd_info_dirty_rate
        );
        register!(self, info, "dump", info_dump::cmd_info_dump);
        register!(self, info, "cpus", info_cpus::cmd_info_cpus);
        register!(
            self,
            info,
            "firmware-log",
            info_firmware_log::cmd_info_firmware_log
        );
        register!(
            self,
            info,
            "hotpluggable-cpus",
            info_hotpluggable_cpus::cmd_info_hotpluggable_cpus
        );
        register!(self, info, "iothreads", info_iothreads::cmd_info_iothreads);
        register!(self, info, "irq", info_irq::cmd_info_irq);
        register!(self, info, "lapic", info_lapic::cmd_info_lapic);
        register!(self, info, "jit", info_jit::cmd_info_jit);
        register!(self, info, "kvm", info_kvm::cmd_info_kvm);
        register!(
            self,
            info,
            "memory_size_summary",
            info_memory_size_summary::cmd_info_memory_size_summary
        );
        register!(
            self,
            info,
            "memory-devices",
            info_memory_devices::cmd_info_memory_devices
        );
        register!(self, info, "mem", info_mem::cmd_info_mem);
        register!(self, info, "memdev", info_memdev::cmd_info_memdev);
        register!(self, info, "pci", info_pci::cmd_info_pci);
        register!(self, info, "pic", info_pic::cmd_info_pic);
        register!(self, info, "qdm", info_qdm::cmd_info_qdm);
        register!(self, info, "qtree", info_qtree::cmd_info_qtree);
        register!(self, info, "qom-tree", info_qom_tree::cmd_info_qom_tree);
        register!(self, info, "mice", info_mice::cmd_info_mice);
        register!(self, info, "migrate", info_migrate::cmd_info_migrate);
        register!(
            self,
            info,
            "migrate_capabilities",
            info_migrate_capabilities::cmd_info_migrate_capabilities
        );
        register!(
            self,
            info,
            "migrate_parameters",
            info_migrate_parameters::cmd_info_migrate_parameters
        );
        register!(self, info, "mtree", info_mtree::cmd_info_mtree);
        register!(self, info, "name", info_name::cmd_info_name);
        register!(self, info, "network", info_network::cmd_info_network);
        register!(self, info, "ramblock", info_ramblock::cmd_info_ramblock);
        register!(self, info, "registers", info_registers::cmd_info_registers);
        register!(self, info, "replay", replay::cmd_info_replay);
        register!(self, info, "rocker", info_rocker::cmd_info_rocker);
        register!(
            self,
            info,
            "rocker-ports",
            info_rocker::cmd_info_rocker_ports
        );
        register!(
            self,
            info,
            "rocker-of-dpa-flows",
            info_rocker::cmd_info_rocker_of_dpa_flows
        );
        register!(
            self,
            info,
            "rocker-of-dpa-groups",
            info_rocker::cmd_info_rocker_of_dpa_groups
        );
        register!(self, info, "roms", info_roms::cmd_info_roms);
        register!(self, info, "sev", info_sev::cmd_info_sev);
        register!(self, info, "sgx", info_sgx::cmd_info_sgx);
        register!(self, info, "snapshots", info_snapshots::cmd_info_snapshots);
        register!(self, info, "spice", info_spice::cmd_info_spice);
        register!(self, info, "skeys", info_skeys::cmd_info_skeys);
        register!(self, info, "stats", info_stats::cmd_info_stats);
        register!(
            self,
            info,
            "sync-profile",
            info_sync_profile::cmd_info_sync_profile
        );
        register!(self, info, "status", info_status::cmd_info_status);
        register!(self, info, "tlb", info_tlb::cmd_info_tlb);
        register!(self, info, "tpm", info_tpm::cmd_info_tpm);
        register!(
            self,
            info,
            "trace-events",
            info_trace_events::cmd_info_trace_events
        );
        register!(self, info, "numa", info_numa::cmd_info_numa);
        register!(self, info, "usb", info_usb::cmd_info_usb);
        register!(self, info, "usbhost", info_usbhost::cmd_info_usbhost);
        register!(self, info, "usernet", info_usernet::cmd_info_usernet);
        register!(self, info, "uuid", info_uuid::cmd_info_uuid);
        register!(self, info, "via", info_via::cmd_info_via);
        register!(
            self,
            info,
            "vcpu_dirty_limit",
            info_vcpu_dirty_limit::cmd_info_vcpu_dirty_limit
        );
        register!(self, info, "version", info_version::cmd_info_version);
        register!(
            self,
            info,
            "virtio-queue-element",
            info_virtio::cmd_info_virtio_queue_element
        );
        register!(
            self,
            info,
            "virtio-status",
            info_virtio::cmd_info_virtio_status
        );
        register!(
            self,
            info,
            "virtio-queue-status",
            info_virtio::cmd_info_virtio_queue_status
        );
        register!(
            self,
            info,
            "virtio-vhost-queue-status",
            info_virtio::cmd_info_virtio_vhost_queue_status
        );
        register!(
            self,
            info,
            "vm-generation-id",
            info_vm_generation_id::cmd_info_vm_generation_id
        );
        register!(self, info, "vnc", info_vnc::cmd_info_vnc);
    }

    /// Associate a handler function with a top-level command name.
    ///
    /// Panics if `name` does not match any entry in the generated command
    /// table — this catches typos at startup rather than silently dropping
    /// the handler.
    pub fn set_main_handler(&mut self, name: &str, handler: HandlerFn) {
        self.main_commands
            .get_mut(name)
            .unwrap_or_else(|| panic!("unknown main command '{name}'"))
            .handler = Some(handler);
    }

    /// Associate a handler function with an `info` subcommand name.
    ///
    /// Panics if `name` does not match any entry in the generated command
    /// table.
    pub fn set_info_handler(&mut self, name: &str, handler: HandlerFn) {
        self.info_commands
            .get_mut(name)
            .unwrap_or_else(|| panic!("unknown info command '{name}'"))
            .handler = Some(handler);
    }

    /// Dispatch a line of user input.
    ///
    /// Returns a [`DispatchOutput`] distinguishing normal output from
    /// a lost connection, so callers can react structurally.
    pub async fn dispatch(&self, conn: &QmpConnection, line: &str, styled: bool) -> DispatchOutput {
        let line = line.trim();
        if line.is_empty() {
            return DispatchOutput::Output(String::new());
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        let cmd = tokens[0];

        // Built-in: help / ?
        if cmd == "help" || cmd == "?" {
            let topic = if tokens.len() > 1 {
                Some(tokens[1])
            } else {
                None
            };
            return DispatchOutput::Output(self.help(topic, styled));
        }

        // info <subcommand>
        if cmd == "info" {
            if tokens.len() < 2 {
                return DispatchOutput::Output(self.help_info());
            }
            let subcmd = tokens[1];
            return self.dispatch_info(conn, subcmd, &tokens[2..]).await;
        }

        // Regular command lookup
        self.dispatch_main(conn, cmd, &tokens[1..]).await
    }

    /// Dispatch a top-level command.
    async fn dispatch_main(
        &self,
        conn: &QmpConnection,
        cmd: &str,
        arg_tokens: &[&str],
    ) -> DispatchOutput {
        // First try exact match.  If that fails and the token contains '/',
        // split at '/' — the part before is the command name and '/' + the
        // rest is a format specifier that becomes the first argument token.
        // This mirrors QEMU's HMP parser which treats '/' as the start of
        // a format argument, e.g. "i/b 0x61" → command "i", args ["/b", "0x61"].
        if let Some(ce) = self.main_commands.get(cmd) {
            return self.run_command(conn, ce, arg_tokens).await;
        }
        if let Some(slash_pos) = cmd.find('/') {
            let (name, fmt) = cmd.split_at(slash_pos);
            if let Some(ce) = self.main_commands.get(name) {
                let mut combined = vec![fmt];
                combined.extend_from_slice(arg_tokens);
                return self.run_command(conn, ce, &combined).await;
            }
        }
        DispatchOutput::Output(format!(
            "Unknown command: '{cmd}'. Type 'help' for a list of commands."
        ))
    }

    /// Dispatch an `info` subcommand.
    async fn dispatch_info(
        &self,
        conn: &QmpConnection,
        subcmd: &str,
        arg_tokens: &[&str],
    ) -> DispatchOutput {
        let Some(ce) = self.info_commands.get(subcmd) else {
            return DispatchOutput::Output(format!(
                "Unknown info subcommand: '{subcmd}'. Type 'help info' for a list."
            ));
        };

        self.run_command(conn, ce, arg_tokens).await
    }

    /// Parse arguments and invoke a command's handler (or report that
    /// the command is not yet implemented).
    async fn run_command(
        &self,
        conn: &QmpConnection,
        ce: &CommandEntry,
        arg_tokens: &[&str],
    ) -> DispatchOutput {
        let Some(handler) = ce.handler else {
            return DispatchOutput::Output(format!(
                "Command '{}' is not yet implemented in the external HMP.",
                ce.entry.name
            ));
        };

        let defs = match parse_arg_defs(ce.entry.args_type) {
            Ok(d) => d,
            Err(e) => {
                return DispatchOutput::Output(format!(
                    "Argument definition error: {e}\nUsage: {} {}\n  {}",
                    ce.entry.name, ce.entry.params, ce.entry.help
                ));
            }
        };
        let args = match parse_args(arg_tokens, &defs) {
            Ok(a) => a,
            Err(e) => {
                return DispatchOutput::Output(format!(
                    "Argument error: {e}\nUsage: {} {}\n  {}",
                    ce.entry.name, ce.entry.params, ce.entry.help
                ));
            }
        };

        match handler(conn, &args).await {
            Ok(output) => DispatchOutput::Output(output),
            Err(CmdError::Disconnected) => DispatchOutput::Disconnected,
            Err(CmdError::Command(e)) => DispatchOutput::Output(format!("Error: {e}")),
        }
    }

    /// Produce help text.
    ///
    /// With no argument, lists all top-level commands.  With a command
    /// name, shows that command's usage and help string.  The special
    /// name "info" lists all info subcommands.
    pub fn help(&self, cmd_name: Option<&str>, styled: bool) -> String {
        match cmd_name {
            None => {
                let mut lines: Vec<String> = Vec::new();
                lines.push("Available commands:".to_string());

                let mut names: Vec<&String> = self.main_commands.keys().collect();
                names.sort();
                // De-duplicate: only show one line per HxEntry name
                let mut seen = std::collections::HashSet::new();
                for name in &names {
                    let ce = &self.main_commands[name.as_str()];
                    if seen.insert(ce.entry.name) {
                        let status = if ce.handler.is_some() { "" } else { " [stub]" };
                        lines.push(format!(
                            "  {:<20} -- {}{}",
                            ce.entry.name, ce.entry.help, status
                        ));
                    }
                }
                lines.push(String::new());
                lines.push("Type 'help <command>' for details.".to_string());
                lines.push("Type 'help info' or 'info' to list info subcommands.".to_string());
                lines.join("\n")
            }
            Some("info") => self.help_info(),
            Some(name) => {
                // Try main commands first, then info commands
                if let Some(ce) = self.main_commands.get(name) {
                    let summary =
                        format!("{} {}\n  {}", ce.entry.name, ce.entry.params, ce.entry.help);
                    if ce.entry.doc.is_empty() {
                        summary
                    } else {
                        format!(
                            "{}\n\n{summary}",
                            crate::format::rst_to_text(ce.entry.doc, styled)
                        )
                    }
                } else if let Some(ce) = self.info_commands.get(name) {
                    let summary = format!(
                        "info {} {}\n  {}",
                        ce.entry.name, ce.entry.params, ce.entry.help
                    );
                    if ce.entry.doc.is_empty() {
                        summary
                    } else {
                        format!(
                            "{}\n\n{summary}",
                            crate::format::rst_to_text(ce.entry.doc, styled)
                        )
                    }
                } else {
                    format!("Unknown command: '{name}'.")
                }
            }
        }
    }

    /// Look up the `args_type` spec for a top-level command.
    pub fn main_args_type(&self, name: &str) -> Option<&'static str> {
        self.main_commands.get(name).map(|ce| ce.entry.args_type)
    }

    /// Look up the `args_type` spec for an `info` subcommand.
    pub fn info_args_type(&self, name: &str) -> Option<&'static str> {
        self.info_commands.get(name).map(|ce| ce.entry.args_type)
    }

    /// Return command names that have a handler registered.
    pub fn implemented_main_commands(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .main_commands
            .iter()
            .filter(|(_, ce)| ce.handler.is_some())
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Return info subcommand names that have a handler registered.
    pub fn implemented_info_commands(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .info_commands
            .iter()
            .filter(|(_, ce)| ce.handler.is_some())
            .map(|(name, _)| name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// List all info subcommands.
    pub fn help_info(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("Available info subcommands:".to_string());

        let mut names: Vec<&String> = self.info_commands.keys().collect();
        names.sort();
        let mut seen = std::collections::HashSet::new();
        for name in &names {
            let ce = &self.info_commands[name.as_str()];
            if seen.insert(ce.entry.name) {
                let status = if ce.handler.is_some() { "" } else { " [stub]" };
                lines.push(format!(
                    "  info {:<16} -- {}{}",
                    ce.entry.name, ce.entry.help, status
                ));
            }
        }
        lines.join("\n")
    }
}
