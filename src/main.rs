/* use bouncy balls for sequencing
*/
#![allow(dead_code)]
#![allow(unused_imports)]
extern crate nannou;
use nannou::prelude::*;
use nannou::ui::prelude::*;
use nannou::ui::Color::Rgba;

extern crate midir; // handle MIDI interfaces
use midir::MidiOutput;

extern crate wmidi; // data-structures to handle MIDI messages
use wmidi::MidiMessage;

extern crate serde_json; // to load/save state
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
extern crate kernel32;
#[cfg(target_os = "windows")]
extern crate user32;
#[cfg(target_os = "windows")]
extern crate winapi;

// use content of src/circle.rs
mod circle;
use circle::Circle;

mod mididata;

use std::collections::{BinaryHeap, HashMap};
use std::convert::TryInto;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use core::cmp::Ordering;

// use content of src/ball.rs
mod ball;
use ball::Ball;

#[cfg(not(target_os = "windows"))]
fn os_specific_things() {}

#[cfg(target_os = "windows")]
fn os_specific_things() {
    // hide console window when running on MS Windows
    let window = unsafe { kernel32::GetConsoleWindow() };

    if window != std::ptr::null_mut() {
        unsafe { user32::ShowWindow(window, winapi::um::winuser::SW_HIDE) };
    }
}

fn main() {
    os_specific_things();
    nannou::app(model_setup).update(update_handler).run();
}

/// Data structure to hold (persistent) model data to track state.
struct Model {
    ui: Ui,           // GUI
    widget_ids: Wids, // IDs of the widgets
    midi: MidiConnector,
    bounce_area_width: f64,
    bounce_area_height: f64,
    bounce_area_center_x: f64,
    bounce_area_center_y: f64,
    ball_model: BallModel, // model data for the ball
    should_display_about: bool,
    should_display_mit_license: bool,
    state: Option<SaveState>,
    do_load_state: bool,  // should state be loaded (before doing GUI things?)
    do_save_state: bool,  // state should be saved (work around borrowing)
    project_name: String, // name of the project used for loading/saving
}

impl Model {
    /// Save / freeze the current state for later export
    pub fn freeze_state(&mut self) {
        // create data (structure) to save
        let pos = self.ball_model.ball.get_position();
        let ball_position_x = pos.x.clone();
        let ball_position_y = pos.y.clone();
        let v = self.ball_model.ball.get_velocity();
        let ball_velocity_x = v.x.clone();
        let ball_velocity_y = v.y.clone();
        let project_name = self.project_name.clone();
        let s = SaveState {
            ball_position_x,
            ball_position_y,
            ball_velocity_x,
            ball_velocity_y,
            project_name,
        };
        self.state = Some(s);
    }
    /// Overwrite model state with given data
    pub fn overwrite_state(&mut self, state: SaveState) {
        // restore data
        let pos = pt2(state.ball_position_x, state.ball_position_y);
        let vel = pt2(state.ball_velocity_x, state.ball_velocity_y);
        self.ball_model.ball.set_position(pos);
        self.ball_model.ball.set_velocity(vel);

        // clear out any old (thus outdated) state
        self.state = None;
    }
}

struct Wids {
    midi_out_ports_list: widget::Id, // drop-down list of MIDI out ports
    menue: MenueWidgets,
    bounce_area: widget::Id,          // where ball can bounce
    ball_control: BallControlWidgets, // control the ball
}

// menue widgets / items
struct MenueWidgets {
    about_button: widget::Id,
    about_text: widget::Id,
    about_text_canvas: widget::Id,
    about_text_close_button: widget::Id,
    mit_license_button: widget::Id,
    mit_license_text: widget::Id,
    mit_license_text_canvas: widget::Id,
    mit_license_text_close_button: widget::Id,
    save_button: widget::Id,
    load_button: widget::Id,
    project_name_textbox: widget::Id,
}

// all things ball control widgets
struct BallControlWidgets {
    velocity_canvas: widget::Id,           // canvas for velocity controls
    velocity_xypad: widget::Id,            // display & control ball velocity
    random_velocity_button: widget::Id,    // a button to randomise the velocity vector
    top: BallInteractionControlWidgets,    // interaction with top border
    right: BallInteractionControlWidgets,  // interaction with right border
    bottom: BallInteractionControlWidgets, // interaction with bottom border
    left: BallInteractionControlWidgets,   // interaction with left border
}

// how the ball interacts
struct BallInteractionControlWidgets {
    widget_canvas: widget::Id, // controls for top border
    note: widget::Id,          // top border control: note
    length: widget::Id,        // top border control: length
    velocity: widget::Id,      // top border control: velocity
    channel: widget::Id,       // top border control: MIDI channel
}

// Ball model
struct BallModel {
    ball: Ball,
    velocity_x: f32,
    velocity_y: f32,
    top_border_interaction: BallInteractionModel,
    right_border_interaction: BallInteractionModel,
    bottom_border_interaction: BallInteractionModel,
    left_border_interaction: BallInteractionModel,
}

struct BallInteractionModel {
    note_display: String, // note to play when ball hits
    midi_note: u8,        // MIDI note to play
    velocity: u8,         // MIDI velocity
    length: u64,          // note length in ms
    midi_channel: u8,     // MIDI channel to send data on
}
// all things MIDI
struct MidiConnector {
    out_port_number: usize,
    out_usable: bool,                            // guard for MidiOutputConnection
    out_connection: midir::MidiOutputConnection, // option seems more intuitive but leads to borrowing issues
    selected_output: String,                     // currently selected MIDI output
    time_queue: BinaryHeap<TimedMidiMessage>,    // (sorted) queue of timestamps to trigger events
}

// a struct to hold timing information and MIDI data
// (to be put in a queue) ... all this to avoid lifetime
// issues with the Model struct (when using wmidi)
// *note* Eq, Order etc. are based soley on the timestamps
#[derive(Hash)]
struct TimedMidiMessage {
    r#type: TimedMidiMessageType,
    note: u8,            // MIDI note
    channel: u8,         // MIDI channel
    velocity: u8,        // note velocity
    timestamp: Duration, // when to trigger
}

// qualify the type of TimedMidiMessage
#[derive(Hash, PartialEq)]
enum TimedMidiMessageType {
    NoteOn,
    NoteOff,
}

impl PartialEq for TimedMidiMessage {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
    }
}

impl Eq for TimedMidiMessage {}

impl PartialOrd for TimedMidiMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.timestamp < other.timestamp {
            return Some(Ordering::Greater); // needs to be processed earlier
        }
        if self.timestamp > other.timestamp {
            return Some(Ordering::Less); // neds to be processed later
        }
        return Some(Ordering::Equal);
    }
}

impl Ord for TimedMidiMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.timestamp < other.timestamp {
            return Ordering::Greater; // needs to be processed earlier
        }
        if self.timestamp > other.timestamp {
            return Ordering::Less; // needs to be processed later
        }
        return Ordering::Equal;
    }
}

impl TimedMidiMessage {
    /// Create note on & off MIDI message.
    pub fn create_on_off(
        chan: u8,
        note: u8,
        velocity: u8,
        trigger: Duration,
        length: Duration,
    ) -> (TimedMidiMessage, TimedMidiMessage) {
        let on = TimedMidiMessage {
            r#type: TimedMidiMessageType::NoteOn,
            note: note,
            channel: chan,
            velocity,
            timestamp: trigger,
        };
        let off = TimedMidiMessage {
            r#type: TimedMidiMessageType::NoteOff,
            note: note,
            channel: chan,
            velocity: 0,
            timestamp: trigger + length,
        };
        return (on, off);
    }

    /// Convert to MIDI message to send
    pub fn to_bytes(self) -> Vec<u8> {
        let mut msg = wmidi::MidiMessage::TuneRequest; // gets reassinged anyway
        let c = wmidi::Channel::from_index(self.channel).expect("could not convert channel");
        unsafe {
            let n = wmidi::Note::from_u8_unchecked(self.note);
            let v = wmidi::U7::from_unchecked(self.velocity);
            if self.r#type == TimedMidiMessageType::NoteOn {
                msg = wmidi::MidiMessage::NoteOn(c, n, v);
            }
            if self.r#type == TimedMidiMessageType::NoteOff {
                msg = wmidi::MidiMessage::NoteOff(c, n, v);
            }
        }
        let mut bytes = vec![0u8; msg.bytes_size()];
        msg.copy_to_slice(bytes.as_mut_slice()).unwrap();
        return bytes;
    }
}

/// What to save
#[derive(Serialize, Deserialize, Clone)]
struct SaveState {
    ball_position_x: f32,
    ball_position_y: f32,
    ball_velocity_x: f32,
    ball_velocity_y: f32,
    project_name: String,
}

/// Create the initial model / state of the application.
fn model_setup(app: &App) -> Model {
    app.set_loop_mode(LoopMode::rate_fps(60.0)); // fixed updates at 60 fps

    // set up the application window
    let _window = app
        .new_window()
        .title(format!("bouncyquencer"))
        .view(view_handler)
        .event(window_event_handler)
        .build()
        .unwrap();

    // set up the GUI
    let mut ui = app.new_ui().build().unwrap();
    let ball_control = BallControlWidgets {
        velocity_canvas: ui.generate_widget_id(),
        velocity_xypad: ui.generate_widget_id(),
        random_velocity_button: ui.generate_widget_id(),
        top: BallInteractionControlWidgets {
            widget_canvas: ui.generate_widget_id(),
            note: ui.generate_widget_id(),
            length: ui.generate_widget_id(),
            velocity: ui.generate_widget_id(),
            channel: ui.generate_widget_id(),
        },
        right: BallInteractionControlWidgets {
            widget_canvas: ui.generate_widget_id(),
            note: ui.generate_widget_id(),
            length: ui.generate_widget_id(),
            velocity: ui.generate_widget_id(),
            channel: ui.generate_widget_id(),
        },
        bottom: BallInteractionControlWidgets {
            widget_canvas: ui.generate_widget_id(),
            note: ui.generate_widget_id(),
            length: ui.generate_widget_id(),
            velocity: ui.generate_widget_id(),
            channel: ui.generate_widget_id(),
        },
        left: BallInteractionControlWidgets {
            widget_canvas: ui.generate_widget_id(),
            note: ui.generate_widget_id(),
            length: ui.generate_widget_id(),
            velocity: ui.generate_widget_id(),
            channel: ui.generate_widget_id(),
        },
    };
    let widget_ids = Wids {
        midi_out_ports_list: ui.generate_widget_id(),
        menue: MenueWidgets {
            about_button: ui.generate_widget_id(),
            about_text: ui.generate_widget_id(),
            about_text_canvas: ui.generate_widget_id(),
            about_text_close_button: ui.generate_widget_id(),
            mit_license_button: ui.generate_widget_id(),
            mit_license_text: ui.generate_widget_id(),
            mit_license_text_canvas: ui.generate_widget_id(),
            mit_license_text_close_button: ui.generate_widget_id(),
            save_button: ui.generate_widget_id(),
            load_button: ui.generate_widget_id(),
            project_name_textbox: ui.generate_widget_id(),
        },
        bounce_area: ui.generate_widget_id(),
        ball_control,
    };

    let bounce_area_width = 200.0;
    let bounce_area_height = 200.0;
    let bounce_area_center_x = 300.0;
    let bounce_area_center_y = 0.0;

    // the bouncy ball
    let mut ball = Ball::new();
    ball.set_color(nannou::color::rgba(
        213.0 / 255.0,
        22.0 / 255.0,
        87.0 / 255.0,
        1.0,
    ));
    ball.set_position(pt2(
        bounce_area_center_x as f32,
        bounce_area_center_y as f32,
    ));
    ball.set_radius(15.0);
    let v = ball.get_velocity();

    let ball_model = BallModel {
        ball,
        velocity_x: v.x,
        velocity_y: v.y,
        top_border_interaction: BallInteractionModel {
            note_display: "None".to_string(),
            midi_note: 128, // outside MIDI note range
            velocity: 64,
            length: 100, // at least 10 ms
            midi_channel: 1,
        },
        right_border_interaction: BallInteractionModel {
            note_display: "None".to_string(),
            midi_note: 128,
            velocity: 64,
            length: 100,
            midi_channel: 1,
        },
        bottom_border_interaction: BallInteractionModel {
            note_display: "None".to_string(),
            midi_note: 128,
            velocity: 64,
            length: 100,
            midi_channel: 1,
        },
        left_border_interaction: BallInteractionModel {
            note_display: "None".to_string(),
            midi_note: 128,
            velocity: 64,
            length: 100,
            midi_channel: 1,
        },
    };
    // all things MIDI
    let midi = MidiConnector {
        out_port_number: 0,
        out_usable: false,
        out_connection: midir::MidiOutput::new("dummy MIDI out")
            .expect("failed to create MIDI output")
            .connect(0, "")
            .expect("could not create dummy connection"),
        selected_output: "no MIDI out selected".to_string(),
        time_queue: BinaryHeap::<TimedMidiMessage>::new(),
    };

    // set up the model
    let model = Model {
        ui,
        widget_ids,
        midi,
        bounce_area_width,
        bounce_area_height,
        bounce_area_center_x,
        bounce_area_center_y,
        ball_model,
        should_display_about: false,
        should_display_mit_license: false,
        state: None,
        do_load_state: false,
        do_save_state: false,
        project_name: "type project name ...".to_string(),
    };

    // set up MIDI output
    let o = MidiOutput::new("bouncyquencer MIDI out"); // '?' operator can't be used
    if let Err(e) = o {
        eprintln!("MIDI out error: {}", e);
        return model;
    }

    return model;
}

/// Handle window events to change the world model.
fn window_event_handler(_app: &App, _model: &mut Model, event: WindowEvent) {
    match event {
        _ => {}
    }
}

/// Handle updates to change the world model and the GUI.
fn update_handler(_app: &App, model: &mut Model, update: Update) {
    if model.do_load_state {
        load_model(model);
        model.do_load_state = false;
    }
    if model.do_save_state {
        model.freeze_state();
        if let Some(s) = &model.state {
            save_model(&s);
        }
        model.do_save_state = false;
    }
    // --- begin GUI code --- //
    let ui = &mut model.ui.set_widgets(); // instantiate widgets

    // bounce area for the ball
    let barea = widget::BorderedRectangle::new([model.bounce_area_width, model.bounce_area_height])
        .x(model.bounce_area_center_x)
        .y(model.bounce_area_center_y)
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 0.3)
        .border(2.0)
        .border_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.5));
    barea.set(model.widget_ids.bounce_area, ui);

    // list of notes
    let mut notenames = Vec::<&str>::new();
    for n in mididata::MIDINOTES.iter().map(|tuple| tuple.0) {
        notenames.push(n);
    }

    //-- start: left control canvas
    widget::Canvas::new()
        .x_relative_to(model.widget_ids.bounce_area, -160.0)
        .y_relative_to(model.widget_ids.bounce_area, 15.0)
        .w_h(90.0, 230.0)
        .rgba(1.0, 0.0, 0.0, 0.0) // canvas area
        .border(0.0) // no visible border
        .title_bar("left")
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .set(model.widget_ids.ball_control.left.widget_canvas, ui);

    // controls for left border (from bottom up for overlay effect)
    for i in widget::DropDownList::new(&mididata::MIDICHANNELS, None)
        .mid_bottom_of(model.widget_ids.ball_control.left.widget_canvas)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .h(28.0) // absolute height
        .label(
            &model
                .ball_model
                .left_border_interaction
                .midi_channel
                .to_string(),
        ) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.left.channel, ui)
    {
        model.ball_model.left_border_interaction.midi_channel = (i + 1).try_into().unwrap();
    }

    // velocity
    let stack_space = 4.0;
    let mut widget_offset = ui
        .wh_of(model.widget_ids.ball_control.left.channel)
        .unwrap()[1]
        + stack_space; // offset based on other widget(s)
    for value in widget::Slider::new(
        model.ball_model.left_border_interaction.velocity as f32,
        0.0,
        127.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.left.widget_canvas, // reference point / widget
        widget_offset,                                    // offset based on other widget
    )
    .h(100.0)
    .label("velocity")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.left.velocity, ui)
    {
        model.ball_model.left_border_interaction.velocity = value as u8;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.left.velocity)
            .unwrap()[1]
        + stack_space;
    for value in widget::Slider::new(
        model.ball_model.left_border_interaction.length as f32,
        10.0,
        5000.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.left.widget_canvas,
        widget_offset,
    )
    .h(30.0)
    .label("length")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.left.length, ui)
    {
        model.ball_model.left_border_interaction.length = value as u64;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.left.length).unwrap()[1]
        + stack_space;
    for i in widget::DropDownList::new(&notenames, None)
        .mid_bottom_with_margin_on(
            model.widget_ids.ball_control.left.widget_canvas,
            widget_offset,
        )
        .h(28.0)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .label(&model.ball_model.left_border_interaction.note_display) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.left.note, ui)
    // attach widget to UI
    {
        // process selection index
        model.ball_model.left_border_interaction.note_display = notenames[i].to_string().clone();
        model.ball_model.left_border_interaction.midi_note = (72 - i).try_into().unwrap();
        // 72 = max MIDI note value
    }
    //-- end: left control canvas

    //-- start: bottom control canvas
    widget::Canvas::new()
        .x_relative_to(model.widget_ids.ball_control.left.widget_canvas, -100.0)
        .y_relative_to(model.widget_ids.ball_control.left.widget_canvas, 0.0)
        .wh_of(model.widget_ids.ball_control.left.widget_canvas)
        .rgba(1.0, 0.0, 0.0, 0.0) // canvas area
        .border(0.0) // no visible border
        .title_bar("bottom")
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .set(model.widget_ids.ball_control.bottom.widget_canvas, ui);

    // controls for bottom border (from bottom up for overlay effect)
    for i in widget::DropDownList::new(&mididata::MIDICHANNELS, None)
        .mid_bottom_of(model.widget_ids.ball_control.bottom.widget_canvas)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .h_of(model.widget_ids.ball_control.left.channel)
        .label(
            &model
                .ball_model
                .bottom_border_interaction
                .midi_channel
                .to_string(),
        ) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.bottom.channel, ui)
    {
        model.ball_model.bottom_border_interaction.midi_channel = (i + 1).try_into().unwrap();
    }

    // velocity
    widget_offset = ui
        .wh_of(model.widget_ids.ball_control.bottom.channel)
        .unwrap()[1]
        + stack_space; // offset based on other widget(s)
    for value in widget::Slider::new(
        model.ball_model.bottom_border_interaction.velocity as f32,
        0.0,
        127.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.bottom.widget_canvas, // reference point / widget
        widget_offset,                                      // offset based on other widget
    )
    .h_of(model.widget_ids.ball_control.left.velocity)
    .label("velocity")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.bottom.velocity, ui)
    {
        model.ball_model.bottom_border_interaction.velocity = value as u8;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.bottom.velocity)
            .unwrap()[1]
        + stack_space;
    for value in widget::Slider::new(
        model.ball_model.bottom_border_interaction.length as f32,
        10.0,
        5000.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.bottom.widget_canvas,
        widget_offset,
    )
    .h_of(model.widget_ids.ball_control.left.length)
    .label("length")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.bottom.length, ui)
    {
        model.ball_model.bottom_border_interaction.length = value as u64;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.bottom.length)
            .unwrap()[1]
        + stack_space;
    for i in widget::DropDownList::new(&notenames, None)
        .mid_bottom_with_margin_on(
            model.widget_ids.ball_control.bottom.widget_canvas,
            widget_offset,
        )
        .h_of(model.widget_ids.ball_control.left.note)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .label(&model.ball_model.bottom_border_interaction.note_display) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.bottom.note, ui)
    // attach widget to UI
    {
        // process selection index
        model.ball_model.bottom_border_interaction.note_display = notenames[i].to_string().clone();
        model.ball_model.bottom_border_interaction.midi_note = (72 - i).try_into().unwrap();
        // 72 = max MIDI note value
    }
    //-- end: bottom control canvas

    //-- start: right control canvas
    widget::Canvas::new()
        .x_relative_to(model.widget_ids.ball_control.bottom.widget_canvas, -100.0)
        .y_relative_to(model.widget_ids.ball_control.left.widget_canvas, 0.0)
        .wh_of(model.widget_ids.ball_control.left.widget_canvas)
        .rgba(1.0, 0.0, 0.0, 0.0) // canvas area
        .border(0.0) // no visible border
        .title_bar("right")
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .set(model.widget_ids.ball_control.right.widget_canvas, ui);

    // controls for right border (from bottom up for overlay effect)
    for i in widget::DropDownList::new(&mididata::MIDICHANNELS, None)
        .mid_bottom_of(model.widget_ids.ball_control.right.widget_canvas)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .h_of(model.widget_ids.ball_control.left.channel)
        .label(
            &model
                .ball_model
                .right_border_interaction
                .midi_channel
                .to_string(),
        ) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.right.channel, ui)
    {
        model.ball_model.right_border_interaction.midi_channel = (i + 1).try_into().unwrap();
    }

    // velocity
    widget_offset = ui
        .wh_of(model.widget_ids.ball_control.right.channel)
        .unwrap()[1]
        + stack_space; // offset based on other widget(s)
    for value in widget::Slider::new(
        model.ball_model.right_border_interaction.velocity as f32,
        0.0,
        127.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.right.widget_canvas, // reference point / widget
        widget_offset,                                     // offset based on other widget
    )
    .h_of(model.widget_ids.ball_control.left.velocity)
    .label("velocity")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.right.velocity, ui)
    {
        model.ball_model.right_border_interaction.velocity = value as u8;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.right.velocity)
            .unwrap()[1]
        + stack_space;
    for value in widget::Slider::new(
        model.ball_model.right_border_interaction.length as f32,
        10.0,
        5000.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.right.widget_canvas,
        widget_offset,
    )
    .h_of(model.widget_ids.ball_control.left.length)
    .label("length")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.right.length, ui)
    {
        model.ball_model.right_border_interaction.length = value as u64;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.right.length)
            .unwrap()[1]
        + stack_space;
    for i in widget::DropDownList::new(&notenames, None)
        .mid_bottom_with_margin_on(
            model.widget_ids.ball_control.right.widget_canvas,
            widget_offset,
        )
        .h_of(model.widget_ids.ball_control.left.note)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .label(&model.ball_model.right_border_interaction.note_display) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.right.note, ui)
    // attach widget to UI
    {
        // process selection index
        model.ball_model.right_border_interaction.note_display = notenames[i].to_string().clone();
        model.ball_model.right_border_interaction.midi_note = (72 - i).try_into().unwrap();
        // 72 = max MIDI note value
    }
    //-- end: right control canvas

    //-- start: top control canvas
    widget::Canvas::new()
        .x_relative_to(model.widget_ids.ball_control.right.widget_canvas, -100.0)
        .y_relative_to(model.widget_ids.ball_control.left.widget_canvas, 0.0)
        .wh_of(model.widget_ids.ball_control.left.widget_canvas)
        .rgba(1.0, 0.0, 0.0, 0.0) // canvas area
        .border(0.0) // no visible border
        .title_bar("top")
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .set(model.widget_ids.ball_control.top.widget_canvas, ui);

    // controls for top border (from bottom up for overlay effect)
    for i in widget::DropDownList::new(&mididata::MIDICHANNELS, None)
        .mid_bottom_of(model.widget_ids.ball_control.top.widget_canvas)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .h_of(model.widget_ids.ball_control.left.channel)
        .label(
            &model
                .ball_model
                .top_border_interaction
                .midi_channel
                .to_string(),
        ) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.top.channel, ui)
    {
        model.ball_model.top_border_interaction.midi_channel = (i + 1).try_into().unwrap();
    }

    // velocity
    widget_offset = ui.wh_of(model.widget_ids.ball_control.top.channel).unwrap()[1] + stack_space; // offset based on other widget(s)
    for value in widget::Slider::new(
        model.ball_model.top_border_interaction.velocity as f32,
        0.0,
        127.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.top.widget_canvas, // reference point / widget
        widget_offset,                                   // offset based on other widget
    )
    .h_of(model.widget_ids.ball_control.left.velocity)
    .label("velocity")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.top.velocity, ui)
    {
        model.ball_model.top_border_interaction.velocity = value as u8;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.top.velocity)
            .unwrap()[1]
        + stack_space;
    for value in widget::Slider::new(
        model.ball_model.top_border_interaction.length as f32,
        10.0,
        5000.0,
    )
    .mid_bottom_with_margin_on(
        model.widget_ids.ball_control.top.widget_canvas,
        widget_offset,
    )
    .h_of(model.widget_ids.ball_control.left.length)
    .label("length")
    .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
    .set(model.widget_ids.ball_control.top.length, ui)
    {
        model.ball_model.top_border_interaction.length = value as u64;
    }

    widget_offset = widget_offset
        + ui.wh_of(model.widget_ids.ball_control.top.length).unwrap()[1]
        + stack_space;
    for i in widget::DropDownList::new(&notenames, None)
        .mid_bottom_with_margin_on(
            model.widget_ids.ball_control.top.widget_canvas,
            widget_offset,
        )
        .h_of(model.widget_ids.ball_control.left.note)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .label(&model.ball_model.top_border_interaction.note_display) // currently selected MIDI note
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.ball_control.top.note, ui)
    // attach widget to UI
    {
        // process selection index
        model.ball_model.top_border_interaction.note_display = notenames[i].to_string().clone();
        model.ball_model.top_border_interaction.midi_note = (72 - i).try_into().unwrap();
        // 72 = max MIDI note value
    }
    //-- end: top control canvas

    // GUI: ball velocity control
    widget::Canvas::new()
        .x_relative_to(model.widget_ids.ball_control.top.widget_canvas, -160.0)
        .w_h(200.0, 265.0)
        .rgba(1.0, 0.0, 0.0, 0.0)
        .border(0.0)
        .label("ball velocity")
        .label_rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8)
        .set(model.widget_ids.ball_control.velocity_canvas, ui);
    for (x, y) in widget::XYPad::new(
        model.ball_model.velocity_x,
        -10.0,
        10.0,
        model.ball_model.velocity_y,
        -10.0,
        10.0,
    )
    .mid_bottom_with_margin_on(model.widget_ids.ball_control.random_velocity_button, 30.0)
    .w_h(200.0, 200.0)
    .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 0.3)
    .label_rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8)
    .border(0.0)
    .set(model.widget_ids.ball_control.velocity_xypad, ui)
    {
        model.ball_model.velocity_x = x;
        model.ball_model.velocity_y = y;
        model.ball_model.ball.set_velocity(Point2::new(x, y));
    }

    // randomise button
    for _click in widget::Button::new()
        .mid_bottom_of(model.widget_ids.ball_control.velocity_canvas)
        .w_h(200.0, 25.0)
        .label("random velocity")
        .label_font_size(15)
        .rgb(0.3, 0.3, 0.3)
        .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
        .border(0.0)
        .set(model.widget_ids.ball_control.random_velocity_button, ui)
    {
        model.ball_model.ball.randomise_velocity();
    }

    // current MIDI out ports list
    let mut port_list = Vec::<String>::new();
    let o = MidiOutput::new("bouncyquencer MIDI out"); // '?' operator can't be used
    if let Err(e) = o {
        eprintln!("MIDI out error: {}", e);
        return;
    }
    let midi_out = o.unwrap();
    for i in 0..midi_out.port_count() {
        let pname = midi_out
            .port_name(i)
            .expect("error retrieving MIDI out port name");
        port_list.push(pname);
    }
    // MIDI out port dropdown list widget -> last for "overlay effect" when selecting
    for i in widget::DropDownList::new(&port_list, None)
        .x(-280.0)
        .y(230.0)
        .border(1.0)
        .border_color(Rgba(1.0, 1.0, 1.0, 0.5)) // TODO: adjust colour to scheme
        .scrollbar_next_to() // scrollbar on the right
        .w(290.0) // absolute width
        .h(25.0) // absolute height
        .label(&model.midi.selected_output) // currently selected port / device
        .label_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 0.8))
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0) // grey selection background
        .set(model.widget_ids.midi_out_ports_list, ui)
    // attach widget to UI
    {
        // process selection index emitted from the list
        model.midi.selected_output = port_list[i].clone();
        model.midi.out_port_number = i;
        model.midi.out_usable = true;
    }

    // the ball
    let border_r = model.bounce_area_width as f32 / 2.0 + model.bounce_area_center_x as f32;
    let border_l = -model.bounce_area_width as f32 / 2.0 + model.bounce_area_center_x as f32;
    let border_t = model.bounce_area_height as f32 / 2.0 + model.bounce_area_center_y as f32;
    let border_b = -model.bounce_area_height as f32 / 2.0 + model.bounce_area_center_y as f32;

    let mut v = model.ball_model.ball.get_velocity();
    let mut pos = model.ball_model.ball.get_position();
    let radius = model.ball_model.ball.get_radius();

    // update the model
    if pos.x + radius > border_r {
        // hit right vertical border -> invert x-component
        v.x = -1.0 * v.x;
        // create MIDI messages & put into send queue
        if "None" != model.ball_model.right_border_interaction.note_display {
            let trig = update.since_start;
            let dur = update.since_start
                + Duration::from_millis(model.ball_model.right_border_interaction.length);
            let (on, off) = TimedMidiMessage::create_on_off(
                model.ball_model.right_border_interaction.midi_channel,
                model.ball_model.right_border_interaction.midi_note,
                model.ball_model.right_border_interaction.velocity,
                trig,
                dur,
            );
            model.midi.time_queue.push(on);
            model.midi.time_queue.push(off);
        }
    }
    if pos.x - radius < border_l {
        // hit left vertical border -> invert x-component
        v.x = -1.0 * v.x;
        // create MIDI messages & put into send queue
        if "None" != model.ball_model.left_border_interaction.note_display {
            let trig = update.since_start;
            let dur = update.since_start
                + Duration::from_millis(model.ball_model.left_border_interaction.length);
            let (on, off) = TimedMidiMessage::create_on_off(
                model.ball_model.left_border_interaction.midi_channel,
                model.ball_model.left_border_interaction.midi_note,
                model.ball_model.left_border_interaction.velocity,
                trig,
                dur,
            );
            model.midi.time_queue.push(on);
            model.midi.time_queue.push(off);
        }
    }
    if pos.y + radius > border_t {
        // hit top horizontal border -> invert y-component
        v.y = -1.0 * v.y;
        // create MIDI messages & put into send queue
        if "None" != model.ball_model.top_border_interaction.note_display {
            let trig = update.since_start;
            let dur = update.since_start
                + Duration::from_millis(model.ball_model.top_border_interaction.length);
            let (on, off) = TimedMidiMessage::create_on_off(
                model.ball_model.top_border_interaction.midi_channel,
                model.ball_model.top_border_interaction.midi_note,
                model.ball_model.top_border_interaction.velocity,
                trig,
                dur,
            );
            model.midi.time_queue.push(on);
            model.midi.time_queue.push(off);
        }
    }
    if pos.y - radius < border_b {
        // hit bottom horizontal border -> invert y-component
        v.y = -1.0 * v.y;
        // create MIDI messages & put into send queue
        if "None" != model.ball_model.bottom_border_interaction.note_display {
            let trig = update.since_start;
            let dur = update.since_start
                + Duration::from_millis(model.ball_model.bottom_border_interaction.length);
            let (on, off) = TimedMidiMessage::create_on_off(
                model.ball_model.bottom_border_interaction.midi_channel,
                model.ball_model.bottom_border_interaction.midi_note,
                model.ball_model.bottom_border_interaction.velocity,
                trig,
                dur,
            );
            model.midi.time_queue.push(on);
            model.midi.time_queue.push(off);
        }
    }

    pos = pos + v; // calculate new position based on velocity
    model.ball_model.ball.set_position(pos); // update position
    model.ball_model.ball.set_velocity(v); // save (new) velocity vector
    model.ball_model.velocity_x = v.x;
    model.ball_model.velocity_y = v.y;

    // --- end GUI code --- //

    // --- begin MIDI code --- //
    if let Some(tm) = model.midi.time_queue.peek() {
        // see if it is (past) time to send message
        if update.since_start >= tm.timestamp {
            if model.midi.out_usable {
                let msg = model.midi.time_queue.pop().unwrap();
                model
                    .midi
                    .out_connection
                    .send(&msg.to_bytes())
                    .expect("could not send MIDI note on message");
            }
        }
    }
    // --- end code --- //

    // --- begin menue
    // -- start GUI project name
    for event in widget::TextBox::new(&model.project_name)
        .x_relative_to(model.widget_ids.midi_out_ports_list, 270.0)
        .y_relative_to(model.widget_ids.midi_out_ports_list, 0.0)
        .w_h(200.0, 25.0)
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
        .border(0.0)
        .text_color(Rgba(255.0 / 255.0, 242.0 / 255.0, 0.0, 1.0))
        .left_justify()
        .set(model.widget_ids.menue.project_name_textbox, ui)
    {
        if let nannou::ui::widget::text_box::Event::Update(txt) = event {
            model.project_name = txt;
        }
    }
    // -- end GUI project name

    // -- start GUI save/load
    for _ in widget::Button::new()
        .x_relative_to(model.widget_ids.menue.project_name_textbox, 170.0)
        .y_relative_to(model.widget_ids.menue.project_name_textbox, 0.0)
        .w_h(75.0, 25.0)
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
        .border(0.0)
        .label("save")
        .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
        .set(model.widget_ids.menue.save_button, ui)
    {
        model.do_save_state = true;
    }

    for _ in widget::Button::new()
        .x_relative_to(model.widget_ids.menue.save_button, 100.0)
        .y_relative_to(model.widget_ids.menue.save_button, 0.0)
        .w_h(75.0, 25.0)
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
        .border(0.0)
        .label("load")
        .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
        .set(model.widget_ids.menue.load_button, ui)
    {
        model.do_load_state = true; // run model state in next run before doing GUI work
    }
    // -- end GUI save/load

    // -- start GUI about
    for _ in widget::Button::new()
        .x_relative_to(model.widget_ids.menue.load_button, 100.0)
        .y_relative_to(model.widget_ids.menue.load_button, 0.0)
        .w_h(75.0, 25.0)
        .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
        .border(0.0)
        .label("about")
        .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
        .set(model.widget_ids.menue.about_button, ui)
    {
        // toggle display about text
        model.should_display_about = !model.should_display_about;
        model.should_display_mit_license = false;
    }

    if model.should_display_about {
        // the canvas to group widgets & control background etc.
        widget::Canvas::new()
            .x(0.0)
            .y(0.0)
            .w_h(730.0, 300.0)
            .set(model.widget_ids.menue.about_text_canvas, ui);

        // actual text(box)
        widget::TextEdit::new(
            "This is bouncyquencer, a MIDI sequencer based on the behaviour of bouncing balls.
You control the speed of the balls and MIDI parameters.

This work builds on the effort of others, namely:
* nannou - https://nannou.cc (licensed under the MIT License)
* nannou_osc - https://nannou.cc (licensed under the MIT License)
* fudi-rs - https://github.com/tpltnt/fudi-rs (licensed under the MIT License)
* rand - https://github.com/rust-random/rand (licensed under the MIT License)
* midir - https://github.com/Boddlnagg/midir (licensed under the MIT License)
* wmidi - https://github.com/wmedrano/wmidi (licensed under the MIT License)
* serde - https://serde.rs (licensed under the MIT License)
* serde_json - https://github.com/serde-rs/json (licensed under the MIT License)",
        )
        .y_relative_to(model.widget_ids.menue.about_text_close_button, 130.0)
        .x(0.0)
        .w(ui.w_of(model.widget_ids.menue.about_text_canvas).unwrap() - 30.0)
        .h(ui.h_of(model.widget_ids.menue.about_text_canvas).unwrap() - 30.0)
        .set(model.widget_ids.menue.about_text, ui);

        // close button for the overlay
        for _ in widget::Button::new()
            .bottom_right_with_margin_on(model.widget_ids.menue.about_text_canvas, 15.0)
            .w_h(75.0, 25.0)
            .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
            .border(0.0)
            .label("close")
            .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
            .set(model.widget_ids.menue.about_text_close_button, ui)
        {
            model.should_display_about = false;
        }

        // MIT license button for the overlay
        for _ in widget::Button::new()
            .bottom_left_with_margin_on(model.widget_ids.menue.about_text_canvas, 15.0)
            .w_h(250.0, 25.0)
            .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
            .border(0.0)
            .label("view MIT license (for crates)")
            .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
            .set(model.widget_ids.menue.mit_license_button, ui)
        {
            model.should_display_mit_license = true;
        }
    }

    if model.should_display_mit_license {
        // the canvas to group widgets & control background etc.
        widget::Canvas::new()
            .x(0.0)
            .y(0.0)
            .w_h(730.0, 400.0)
            .set(model.widget_ids.menue.mit_license_text_canvas, ui);

        // actual text(box)
        widget::TextEdit::new(
                "Copyright <YEAR> <COPYRIGHT HOLDER>

    Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the 'Software'), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.",
            )
            .y_relative_to(model.widget_ids.menue.mit_license_text_close_button, 170.0)
            .set(model.widget_ids.menue.mit_license_text , ui);

        // close button for the overlay
        for _ in widget::Button::new()
            .mid_bottom_with_margin_on(model.widget_ids.menue.mit_license_text_canvas, 15.0)
            .w_h(75.0, 25.0)
            .rgba(119.0 / 255.0, 129.0 / 255.0, 135.0 / 255.0, 1.0)
            .border(0.0)
            .label("close")
            .label_rgb(255.0 / 255.0, 242.0 / 255.0, 0.0)
            .set(model.widget_ids.menue.mit_license_text_close_button, ui)
        {
            model.should_display_mit_license = false;
        }
    }
    // -- end GUI about

    // --- end menue
}

/// Draw model state on the screen.
fn view_handler(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    draw.background()
        .rgb(29.0 / 255.0, 43.0 / 255.0, 44.0 / 255.0); // black-ish background
    model.ball_model.ball.display(&draw); // draw ball
    draw.to_frame(app, &frame).unwrap(); // draw app content
    model.ui.draw_to_frame(app, &frame).unwrap(); // draw UI
}

/// Save the significant parts of the model (state).
fn save_model(state: &SaveState) {
    // write out to file
    let fname = state.project_name.clone() + ".state";
    let data = serde_json::to_string(&state).expect("could not serialize data");
    let fpath = Path::new(&fname);
    fs::write(fpath, data).expect("could not save data");
}

/// Load the significant parts of the model (state).
fn load_model(model: &mut Model) {
    // load state
    let fname = model.project_name.clone() + ".state";
    let sdata = fs::read_to_string(fname).expect("could not read state data file");
    let state: SaveState = serde_json::from_str(&sdata).expect("could not de-serialize data");
    model.overwrite_state(state);
}
