use wrum_net::store;
use wrum_proto::{ClientMsg, NameError, ServerMsg};

use crate::scene::{Scene, menu::MenuScene};

use raylib::{
    RaylibHandle,
    consts::{KeyboardKey, MouseButton},
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::Color,
    texture::Texture2D,
};

const CANVAS_W: i32 = 320;
const CANVAS_H: i32 = 180;

const LOGO_POS: (i32, i32) = (16, 10);
const PANEL: (i32, i32, i32, i32) = (48, 32, 224, 142);

const BOX_W: i32 = 200;
const BOX_H: i32 = 22;
const BOX_X: i32 = (CANVAS_W - BOX_W) / 2;

const NAME_Y: i32 = 64;
const COUNTRY_Y: i32 = 108;

const BTN_Y: i32 = 138;
const BTN_H: i32 = 18;
const BTN_W: i32 = 96;

const REGISTER_BTN: (i32, i32, i32, i32) = (BOX_X, BTN_Y, BTN_W, BTN_H);
const GUEST_BTN: (i32, i32, i32, i32) = (BOX_X + BOX_W - BTN_W, BTN_Y, BTN_W, BTN_H);

const NAME_BOX: (i32, i32, i32, i32) = (BOX_X, NAME_Y, BOX_W, BOX_H);
const COUNTRY_BOX: (i32, i32, i32, i32) = (BOX_X, COUNTRY_Y, BOX_W, BOX_H);

const CELL: i32 = 32;
const STRIP_X: i32 = BOX_X + (BOX_W - CELL * 5) / 2;

const MIN_LEN: usize = 3;
const MAX_LEN: usize = 16;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

const BG: Color = rgb(0, 0, 0);
const CARD: Color = rgb(20, 24, 36);
const STRIPE: Color = rgb(25, 30, 46);
const BORDER: Color = rgb(48, 56, 80);
const TEXT: Color = rgb(228, 232, 242);
const DIM: Color = rgb(122, 132, 154);
const ACCENT: Color = rgb(255, 255, 255);
const ACCENT_HI: Color = rgb(255, 255, 255);
const ACCENT_LO: Color = rgb(200, 200, 200);
const DARK: Color = rgb(22, 18, 6);
const BTN: Color = rgb(34, 42, 64);
const BTN_HI: Color = rgb(46, 58, 88);
const RED: Color = rgb(240, 90, 90);

const WHITE: Color = ACCENT;

const COUNTRIES: &str = "FI";

fn country_count() -> usize {
    COUNTRIES.split_whitespace().count()
}
fn country_code(i: usize) -> &'static str {
    COUNTRIES.split_whitespace().nth(i).unwrap_or("??")
}
fn country_index(c: [char; 2]) -> usize {
    let s: String = c.iter().map(|c| c.to_ascii_uppercase()).collect();
    COUNTRIES
        .split_whitespace()
        .position(|x| x == s)
        .unwrap_or(0)
}
fn country_chars(i: usize) -> [char; 2] {
    let mut it = country_code(i).chars();
    [it.next().unwrap_or('?'), it.next().unwrap_or('?')]
}

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Register,
    Update,
}

pub struct Auth {
    pub name: String,
    pub country: [char; 2],
}

#[derive(Default)]
pub struct AuthScene {
    state: State,
    logo: Option<Texture2D>,
    mode: Mode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Country,
}

#[derive(Default)]
enum State {
    #[default]
    Unknown,
    NeedName {
        input: String,
        field: Field,
        idx: usize,
        buf: String,
        error: Option<&'static str>,
        shake: f32,
        pending: bool,
        guest_name: String,
        guest_country: [char; 2],
    },
}

fn valid_name(s: &str) -> Option<String> {
    let t = s.trim();
    let n = t.chars().count();
    ((MIN_LEN..=MAX_LEN).contains(&n) && wrum_proto::valid_name(t).is_some()).then(|| t.to_string())
}
fn allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ')
}

/// (text, is_problem)
fn hint(input: &str) -> (String, bool) {
    let t = input.trim();
    let n = t.chars().count();
    if input.is_empty() {
        ("letters, digits, _ - or space".into(), false)
    } else if n < MIN_LEN {
        (
            format!(
                "{} more character{}",
                MIN_LEN - n,
                if MIN_LEN - n == 1 { "" } else { "s" }
            ),
            false,
        )
    } else if wrum_proto::valid_name(t).is_none() {
        ("okay that's inappropriate".into(), true)
    } else {
        ("ok".into(), false)
    }
}

fn mouse_canvas(rl: &RaylibHandle) -> (f32, f32) {
    let m = rl.get_mouse_position();
    (
        m.x * CANVAS_W as f32 / rl.get_screen_width().max(1) as f32,
        m.y * CANVAS_H as f32 / rl.get_screen_height().max(1) as f32,
    )
}
fn hit(r: (i32, i32, i32, i32), p: (f32, f32)) -> bool {
    p.0 >= r.0 as f32 && p.0 < (r.0 + r.2) as f32 && p.1 >= r.1 as f32 && p.1 < (r.1 + r.3) as f32
}
fn centered(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    text: &str,
    y: i32,
    size: i32,
    color: Color,
) {
    let w = tm.measure_text(text, size);
    tm.draw_text(text, (CANVAS_W - w) / 2, y, size, color);
}

impl AuthScene {
    pub fn update_mode(name: String, country: [char; 2]) -> AuthScene {
        Self {
            state: State::NeedName {
                input: name.clone(),
                field: Field::Name,
                idx: country_index(country),
                buf: String::new(),
                error: None,
                shake: 0.,
                pending: false,
                guest_name: name,
                guest_country: country,
            },
            logo: None,
            mode: Mode::Update,
        }
    }
}

impl Scene for AuthScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32, _alpha: f32) -> super::Transition {
        let n = country_count();
        let mouse = mouse_canvas(&ctx.rl);
        let clicked = ctx
            .rl
            .is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let mut go_guest = false;

        if let State::NeedName {
            input,
            field,
            idx,
            buf,
            error,
            shake,
            pending,
            ..
        } = &mut self.state
        {
            *shake = (*shake - dt).max(0.0);
            let mut submit = false;

            if !*pending {
                if clicked {
                    if hit(NAME_BOX, mouse) {
                        *field = Field::Name;
                        buf.clear();
                    } else if hit(COUNTRY_BOX, mouse) {
                        if *field == Field::Country {
                            let k = if mouse.0 < STRIP_X as f32 {
                                -1
                            } else if mouse.0 >= (STRIP_X + CELL * 5) as f32 {
                                1
                            } else {
                                (mouse.0 as i32 - STRIP_X) / CELL - 2
                            };
                            *idx = (*idx as i32 + k).rem_euclid(n as i32) as usize;
                        }
                        *field = Field::Country;
                        buf.clear();
                    } else if hit(REGISTER_BTN, mouse) {
                        submit = true;
                    } else if hit(GUEST_BTN, mouse) {
                        go_guest = true;
                    }
                }

                if ctx.rl.is_key_pressed(KeyboardKey::KEY_TAB)
                    || ctx.rl.is_key_pressed(KeyboardKey::KEY_DOWN)
                    || ctx.rl.is_key_pressed(KeyboardKey::KEY_UP)
                {
                    *field = match *field {
                        Field::Name => Field::Country,
                        Field::Country => Field::Name,
                    };
                    buf.clear();
                }

                match *field {
                    Field::Name => {
                        while let Some(c) = ctx.rl.get_char_pressed() {
                            if allowed(c) && input.chars().count() < MAX_LEN {
                                input.push(c);
                                *error = None;
                            }
                        }
                        if ctx.rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE)
                            || ctx.rl.is_key_pressed_repeat(KeyboardKey::KEY_BACKSPACE)
                        {
                            if ctx.rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL) {
                                input.clear();
                            } else {
                                input.pop();
                            }
                            *error = None;
                        }
                    }
                    Field::Country => {
                        if ctx.rl.is_key_pressed(KeyboardKey::KEY_RIGHT)
                            || ctx.rl.is_key_pressed_repeat(KeyboardKey::KEY_RIGHT)
                        {
                            *idx = (*idx + 1) % n;
                            buf.clear();
                        }
                        if ctx.rl.is_key_pressed(KeyboardKey::KEY_LEFT)
                            || ctx.rl.is_key_pressed_repeat(KeyboardKey::KEY_LEFT)
                        {
                            *idx = (*idx + n - 1) % n;
                            buf.clear();
                        }

                        while let Some(c) = ctx.rl.get_char_pressed() {
                            if !c.is_ascii_alphabetic() {
                                continue;
                            }
                            if buf.len() >= 2 {
                                buf.clear();
                            }
                            buf.push(c.to_ascii_uppercase());
                            if let Some(p) = COUNTRIES
                                .split_whitespace()
                                .position(|x| x.starts_with(buf.as_str()))
                            {
                                *idx = p;
                            }
                            if buf.len() == 2 {
                                buf.clear();
                            }
                        }
                    }
                }

                if ctx.rl.is_key_pressed(KeyboardKey::KEY_ENTER)
                    || ctx.rl.is_key_pressed(KeyboardKey::KEY_KP_ENTER)
                {
                    submit = true;
                }
            }

            if submit {
                match valid_name(input) {
                    Some(name) => {
                        *pending = true;
                        *error = None;
                        let _ = ctx.net.send(&match self.mode {
                            Mode::Register => ClientMsg::Register {
                                name,
                                country: country_chars(*idx),
                            },
                            Mode::Update => ClientMsg::Update {
                                country: country_chars(*idx),
                                name,
                            },
                        });
                    }
                    None => {
                        *error = Some("3-16 letters, digits, _ - or space");
                        *shake = 0.35;
                        *field = Field::Name;
                    }
                }
            }
        }

        if go_guest {
            if let State::NeedName {
                guest_name,
                guest_country,
                ..
            } = &self.state
            {
                return super::Transition::Switch(Box::new(MenuScene::new(
                    "",
                    ctx,
                    Auth {
                        name: guest_name.clone(),
                        country: *guest_country,
                    },
                )));
            }
        }
        super::Transition::None
    }
    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let t = ctx.rl.get_time() as f32;

        if self.logo.is_none() {
            self.logo = ctx.rl.load_texture(ctx.thread, "assets/logo.png").ok();
        }

        let n = country_count();
        let mouse = mouse_canvas(&ctx.rl);
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);

        tm.clear_background(BG);

        if let Some(logo) = &self.logo {
            tm.draw_texture(logo, LOGO_POS.0, LOGO_POS.1, WHITE);
        }

        match &self.state {
            State::Unknown => {
                let dots = ".".repeat(((t * 3.0) as usize % 3) + 1);
                centered(&mut tm, &format!("logging in{dots}"), 85, 10, DIM);
            }

            State::NeedName {
                input,
                field,
                idx,
                error,
                shake,
                pending,
                ..
            } => {
                tm.draw_rectangle(PANEL.0, PANEL.1, PANEL.2, PANEL.3, CARD);
                tm.draw_rectangle_lines(PANEL.0, PANEL.1, PANEL.2, PANEL.3, BORDER);

                tm.draw_text(
                    match self.mode {
                        Mode::Register => "CREATE ACCOUNT",
                        Mode::Update => "UPDATE ACCOUNT",
                    },
                    BOX_X,
                    40,
                    8,
                    DIM,
                );

                tm.draw_rectangle(BOX_X, 50, BOX_W, 1, BORDER);

                let off = if *shake > 0.0 {
                    ((t * 70.0).sin() * 4.0 * (*shake / 0.35)) as i32
                } else {
                    0
                };

                let bx = BOX_X + off;
                let name_focus = *field == Field::Name;

                tm.draw_text("name", BOX_X, NAME_Y - 10, 8, DIM);
                frame(
                    &mut tm,
                    bx,
                    NAME_Y,
                    name_focus,
                    error.is_some().then_some(RED),
                );

                let ty = NAME_Y + (BOX_H - 10) / 2;

                if input.is_empty() {
                    tm.draw_text("your name", bx + 8, ty, 10, DIM);
                } else {
                    tm.draw_text(input, bx + 8, ty, 10, TEXT);
                }

                if name_focus && !*pending && (t * 2.0) as i32 % 2 == 0 {
                    let cx = bx + 8 + tm.measure_text(input, 10) + 1;
                    tm.draw_rectangle(cx, ty - 1, 1, 12, ACCENT);
                }

                let line_y = NAME_Y + BOX_H + 5;

                match error {
                    Some(e) => tm.draw_text(e, bx, line_y, 8, RED),
                    None => {
                        let (h, bad) = hint(input);
                        tm.draw_text(&h, bx, line_y, 8, if bad { RED } else { DIM });
                    }
                }

                let count = format!("{}/{}", input.chars().count(), MAX_LEN);
                let cw = tm.measure_text(&count, 8);

                tm.draw_text(&count, BOX_X + BOX_W - cw, line_y, 8, DIM);

                let c_focus = *field == Field::Country;
                tm.draw_text("country", BOX_X, COUNTRY_Y - 10, 8, DIM);

                frame(&mut tm, BOX_X, COUNTRY_Y, c_focus, None);

                let cy = COUNTRY_Y + (BOX_H - 10) / 2;

                if c_focus {
                    for k in -2i32..=2 {
                        let i = (*idx as i32 + k).rem_euclid(n as i32) as usize;
                        let code = country_code(i);
                        let x = STRIP_X + (k + 2) * CELL;
                        let w = tm.measure_text(code, 10);

                        if k == 0 {
                            tm.draw_rectangle(x, COUNTRY_Y + 3, CELL, BOX_H - 6, ACCENT);
                            tm.draw_text(code, x + (CELL - w) / 2, cy, 10, DARK);
                        } else {
                            tm.draw_text(code, x + (CELL - w) / 2, cy, 10, DIM);
                        }
                    }

                    tm.draw_text("<", BOX_X + 5, cy, 10, TEXT);
                    tm.draw_text(">", BOX_X + BOX_W - 11, cy, 10, TEXT);
                } else {
                    tm.draw_text(country_code(*idx), BOX_X + 8, cy, 10, TEXT);
                }

                let ready = valid_name(input).is_some() && !*pending;

                button(
                    &mut tm,
                    REGISTER_BTN,
                    match self.mode {
                        Mode::Register => {
                            if *pending {
                                "REGISTERING..."
                            } else {
                                "REGISTER"
                            }
                        }
                        Mode::Update => {
                            if *pending {
                                "UPDATING..."
                            } else {
                                "UPDATE"
                            }
                        }
                    },
                    ready,
                    hit(REGISTER_BTN, mouse),
                    true,
                );

                button(
                    &mut tm,
                    GUEST_BTN,
                    match self.mode {
                        Mode::Register => "PLAY AS GUEST",
                        Mode::Update => "CANCEL",
                    },
                    !*pending,
                    hit(GUEST_BTN, mouse),
                    false,
                );
            }
        }
    }

    fn on_message(
        &mut self,
        ctx: &mut crate::ctx::Context,
        msg: &wrum_proto::ServerMsg,
    ) -> super::Transition {
        match msg {
            ServerMsg::HelloOk {
                name,
                country,
                guest,
            } => {
                if *guest {
                    self.state = State::NeedName {
                        input: String::new(),
                        field: Field::Name,
                        idx: country_index(*country),
                        buf: String::new(),
                        error: None,
                        shake: 0.0,
                        pending: false,
                        guest_name: name.clone(),
                        guest_country: *country,
                    };
                } else {
                    return super::Transition::Switch(Box::new(MenuScene::new(
                        "",
                        ctx,
                        Auth {
                            name: name.clone(),
                            country: *country,
                        },
                    )));
                }
            }
            ServerMsg::Registered {
                token,
                name,
                country,
            } => {
                store::save("token", token);
                return super::Transition::Switch(Box::new(MenuScene::new(
                    "",
                    ctx,
                    Auth {
                        name: name.clone(),
                        country: *country,
                    },
                )));
            }
            ServerMsg::NameRejected(e) => {
                if let State::NeedName {
                    error,
                    shake,
                    pending,
                    field,
                    ..
                } = &mut self.state
                {
                    *pending = false;
                    *shake = 0.35;
                    *field = Field::Name;
                    *error = Some(match e {
                        NameError::Taken => "name already taken",
                        NameError::Invalid => "3-16 letters, digits, _, - or space",
                        NameError::Busy => "try again later",
                    });
                }
            }
            ServerMsg::Updated { name, country } => {
                return super::Transition::Switch(Box::new(MenuScene::new(
                    "",
                    ctx,
                    Auth {
                        name: name.clone(),
                        country: *country,
                    },
                )));
            }
            _ => {}
        }
        super::Transition::None
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}

fn frame(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    x: i32,
    y: i32,
    focused: bool,
    color: Option<Color>,
) {
    tm.draw_rectangle(x, y, BOX_W, BOX_H, CARD);

    let border = color.unwrap_or(if focused { ACCENT } else { BORDER });

    tm.draw_rectangle_lines(x, y, BOX_W, BOX_H, border);
}
fn button(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    r: (i32, i32, i32, i32),
    label: &str,
    enabled: bool,
    hovered: bool,
    primary: bool,
) {
    let (x, y, w, h) = r;

    let (face, edge, ink) = match (primary, enabled) {
        (_, false) => (BTN, BORDER, DIM),
        (true, true) => (if hovered { ACCENT_HI } else { ACCENT }, ACCENT_LO, DARK),
        (false, true) => (if hovered { BTN_HI } else { BTN }, BORDER, TEXT),
    };

    let lift = if hovered && enabled { 0 } else { 1 };

    tm.draw_rectangle(x, y + 2, w, h - 1, edge);
    tm.draw_rectangle(x, y + 2 - lift, w, h - 2, face);

    let text_w = tm.measure_text(label, 8);
    tm.draw_text(
        label,
        x + (w - text_w) / 2,
        y + (h - 8) / 2 + 2 - lift,
        8,
        ink,
    );
}
