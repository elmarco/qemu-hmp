// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use qapi::qmp::{InputBtnEvent, InputButton, InputEvent};

use crate::args::ArgValue;
use crate::commands::{require_int, CmdError};
use crate::qmp::QmpConnection;

/// Bitmask constants matching QEMU's MOUSE_EVENT_* defines.
const MOUSE_EVENT_LBUTTON: i64 = 0x01;
const MOUSE_EVENT_MBUTTON: i64 = 0x04;
const MOUSE_EVENT_RBUTTON: i64 = 0x02;

/// Tracks the cumulative button state between calls, matching the
/// `static int mouse_button_state` in QEMU's ui/ui-hmp-cmds.c.
static BUTTON_STATE: AtomicI64 = AtomicI64::new(0);

/// Map of (InputButton, bitmask) for the three standard mouse buttons.
const BUTTON_MAP: &[(InputButton, i64)] = &[
    (InputButton::left, MOUSE_EVENT_LBUTTON),
    (InputButton::middle, MOUSE_EVENT_MBUTTON),
    (InputButton::right, MOUSE_EVENT_RBUTTON),
];

pub async fn cmd_mouse_button(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let new_state = require_int(args, "button_state")?;
    let old_state = BUTTON_STATE.swap(new_state, Ordering::Relaxed);

    if old_state == new_state {
        return Ok(String::new());
    }

    // Compute press/release events for each button whose state changed.
    let mut events = Vec::new();
    for &(button, mask) in BUTTON_MAP {
        let was_down = (old_state & mask) != 0;
        let is_down = (new_state & mask) != 0;
        if was_down != is_down {
            events.push(InputEvent::btn(
                InputBtnEvent {
                    button,
                    down: is_down,
                }
                .into(),
            ));
        }
    }

    if !events.is_empty() {
        conn.execute(qapi::qmp::input_send_event {
            device: None,
            head: None,
            events,
        })
        .await
        .map_err(CmdError::from)?;
    }

    Ok(String::new())
}
