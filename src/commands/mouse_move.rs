// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::HashMap;

use qapi::qmp::{InputAxis, InputBtnEvent, InputButton, InputEvent, InputMoveEvent};

use crate::args::{parse_int, ArgValue};
use crate::commands::{require_str, CmdError};
use crate::qmp::QmpConnection;

pub async fn cmd_mouse_move(
    conn: &QmpConnection,
    args: &HashMap<String, ArgValue>,
) -> Result<String, CmdError> {
    let dx_str = require_str(args, "dx_str")?;
    let dy_str = require_str(args, "dy_str")?;

    let dx = parse_int(&dx_str).map_err(CmdError::Command)?;
    let dy = parse_int(&dy_str).map_err(CmdError::Command)?;

    // Determine scroll button from optional dz argument.
    let scroll_button = if let Some(ArgValue::Str(dz_str)) = args.get("dz_str") {
        let dz = parse_int(dz_str).map_err(CmdError::Command)?;
        if dz > 0 {
            Some(InputButton::wheel_up)
        } else if dz < 0 {
            Some(InputButton::wheel_down)
        } else {
            None
        }
    } else {
        None
    };

    let mut events: Vec<InputEvent> = vec![
        InputEvent::rel(
            InputMoveEvent {
                axis: InputAxis::x,
                value: dx,
            }
            .into(),
        ),
        InputEvent::rel(
            InputMoveEvent {
                axis: InputAxis::y,
                value: dy,
            }
            .into(),
        ),
    ];

    if let Some(button) = scroll_button {
        events.push(InputEvent::btn(InputBtnEvent { button, down: true }.into()));
    }

    conn.execute(qapi::qmp::input_send_event {
        device: None,
        head: None,
        events,
    })
    .await
    .map_err(CmdError::from)?;

    // Release scroll button in a separate sync (matching the C code's
    // two separate qemu_input_event_sync() calls).
    if let Some(button) = scroll_button {
        conn.execute(qapi::qmp::input_send_event {
            device: None,
            head: None,
            events: vec![InputEvent::btn(
                InputBtnEvent {
                    button,
                    down: false,
                }
                .into(),
            )],
        })
        .await
        .map_err(CmdError::from)?;
    }

    Ok(String::new())
}
