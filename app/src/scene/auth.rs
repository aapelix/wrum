use wrum_net::store;
use wrum_proto::{ClientMsg, CodeError, DeviceInfo, NameError, ServerMsg};

use crate::{
    ctx,
    scene::{Scene, Transition, menu::MenuScene},
    ui::{
        Button,
        input::{
            BOX_H, BOX_W, BOX_X, CountryPicker, DIM, Focus, Rect, SHAKE_TIME, TEXT, TextField,
            centered, enter_pressed, header, hit, mouse_canvas,
        },
    },
};

use raylib::{
    consts::MouseButton,
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::Color,
};

const NAME_Y: i32 = 44;
const COUNTRY_Y: i32 = 88;
const ALT_Y: i32 = 162;
const ALT_LEFT: Rect = (BOX_X, ALT_Y, BTN_W, BTN_H);
const ALT_RIGHT: Rect = (BOX_X + BOX_W - BTN_W, ALT_Y, BTN_W, BTN_H);
const RESEND: Rect = (BOX_X, NAME_Y + BOX_H + 12, 110, 12);

const DEV_Y: i32 = 40;
const DEV_ROW: i32 = 14;
const MAX_DEV: usize = 5;

const BTN_Y: i32 = 118;
const BTN_H: i32 = 18;
const BTN_W: i32 = 96;

const PRIMARY_BTN: Rect = (BOX_X, BTN_Y, BTN_W, BTN_H);
const SECONDARY_BTN: Rect = (BOX_X + BOX_W - BTN_W, BTN_Y, BTN_W, BTN_H);

const MIN_LEN: usize = 3;
const MAX_LEN: usize = 16;

const NAME_RULES: &str = "3-16 letters, digits, _ - or space";

const F_NAME: usize = 0;
const F_COUNTRY: usize = 1;

const RED: Color = Color {
    r: 240,
    g: 90,
    b: 90,
    a: 255,
};

fn valid_name(s: &str) -> Option<String> {
    let t = s.trim();
    let n = t.chars().count();
    ((MIN_LEN..=MAX_LEN).contains(&n) && wrum_proto::valid_name(t).is_some()).then(|| t.to_string())
}

fn name_char(c: char) -> bool {
    c.is_alphabetic() || c.is_ascii_digit() || matches!(c, '_' | '-' | ' ')
}
fn email_char(c: char) -> bool {
    c.is_ascii_graphic()
}
fn code_char(c: char) -> bool {
    c.is_ascii_digit()
}

fn valid_email(s: &str) -> bool {
    let t = s.trim();
    match t.split_once('@') {
        Some((user, host)) => !user.is_empty() && host.contains('.') && !host.ends_with('.'),
        None => false,
    }
}

fn name_hint(input: &str) -> (String, bool) {
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

#[derive(Clone)]
pub struct Auth {
    pub name: String,
    pub country: [char; 2],
    pub email: Option<String>,
}

pub struct AuthScene {
    state: State,
    button: Button,
    clock: f32,
    auth: Auth,
    logged_in: bool,
    verified: Option<String>,
    started: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum AccountMode {
    Create,
    Update,
}
impl AccountMode {
    fn title(self) -> &'static str {
        match self {
            Self::Create => "Create account",
            Self::Update => "Update account",
        }
    }
    fn button_labels(self, pending: bool) -> (&'static str, &'static str) {
        match (self, pending) {
            (Self::Create, false) => ("REGISTER", "PLAY AS GUEST"),
            (Self::Create, true) => ("REGISTERING...", "PLAY AS GUEST"),
            (Self::Update, false) => ("UPDATE", "CANCEL"),
            (Self::Update, true) => ("UPDATING...", "CANCEL"),
        }
    }
    fn message(self, name: String, country: [char; 2]) -> ClientMsg {
        match self {
            Self::Create => ClientMsg::Register { name, country },
            Self::Update => ClientMsg::Update { name, country },
        }
    }
}

struct AccountState {
    name: TextField,
    country: CountryPicker,
    focus: Focus,
    shake: f32,
    pending: bool,
    mode: AccountMode,
}
impl AccountState {
    fn create(country: [char; 2]) -> Self {
        Self {
            name: TextField::new("name", NAME_Y, MAX_LEN, name_char)
                .with_placeholder("your name")
                .with_hint(name_hint),
            country: CountryPicker::new("country", COUNTRY_Y, country),
            focus: Focus::default(),
            shake: 0.0,
            pending: false,
            mode: AccountMode::Create,
        }
    }
    fn update(old_name: String, old_country: [char; 2]) -> Self {
        Self {
            name: TextField::new("name", NAME_Y, MAX_LEN, name_char)
                .with_value(old_name.clone())
                .with_placeholder(old_name)
                .with_hint(name_hint),
            country: CountryPicker::new("country", COUNTRY_Y, old_country),
            focus: Focus::default(),
            shake: 0.0,
            pending: false,
            mode: AccountMode::Update,
        }
    }
    fn reject(&mut self, msg: &'static str) {
        self.pending = false;
        self.shake = SHAKE_TIME;
        self.focus.set(F_NAME);
        self.name.error = Some(msg);
    }
}

enum State {
    Idle,
    Email {
        field: TextField,
        pending: bool,
    },
    Code {
        email: String,
        field: TextField,
        resend_at: f32,
        pending: bool,
    },
    Account(AccountState),
    Link {
        code: Option<String>,
        expires: f32,
    },
    LinkEntry {
        field: TextField,
        pending: bool,
    },
    Devices {
        list: Option<Vec<DeviceInfo>>,
    },
}
impl State {
    fn email() -> State {
        State::Email {
            field: TextField::new("", NAME_Y + 3, 64, email_char)
                .with_placeholder("you@example.com"),
            pending: false,
        }
    }
    fn code(email: String, resend_at: f32) -> State {
        State::Code {
            email,
            field: TextField::new("", NAME_Y, 6, code_char).with_placeholder("000000"),
            resend_at,
            pending: false,
        }
    }
    fn link_entry() -> State {
        State::LinkEntry {
            field: TextField::new("link code", NAME_Y, 8, |c| c.is_ascii_alphanumeric())
                .with_placeholder("code from your other device"),
            pending: false,
        }
    }
}

fn to_menu(
    ctx: &mut ctx::Context,
    name: String,
    country: [char; 2],
    email: Option<String>,
) -> Transition {
    Transition::Switch(Box::new(MenuScene::new(
        "",
        ctx,
        Auth {
            name,
            country,
            email,
        },
    )))
}
fn single_update(rl: &mut raylib::RaylibHandle, field: &mut TextField) -> bool {
    field.update(rl, true);
    enter_pressed(rl)
}

fn ago(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    match (now - ts).max(0) {
        0..=59 => "just now".into(),
        s @ 60..=3599 => format!("{}m ago", s / 60),
        s @ 3600..=86399 => format!("{}h ago", s / 3600),
        s => format!("{}d ago", s / 86400),
    }
}

fn code_error_text(e: &CodeError, link: bool) -> &'static str {
    match e {
        CodeError::BadEmail => "invalid email",
        CodeError::Busy => "try again later",
        CodeError::Expired => "code expired, request a new one",
        CodeError::TooSoon => "wait a minute before requesting",
        CodeError::Wrong if link => "invalid or expired code",
        CodeError::Wrong => "wrong code",
    }
}

impl AuthScene {
    fn make(ctx: &mut ctx::Context, state: State, auth: Auth, logged_in: bool) -> Self {
        Self {
            state,
            button: Button::new(ctx),
            clock: 0.0,
            auth,
            logged_in,
            verified: None,
            started: false,
        }
    }
    pub fn new(ctx: &mut ctx::Context) -> Self {
        Self::make(
            ctx,
            State::Idle,
            Auth {
                name: String::new(),
                country: ['F', 'I'],
                email: None,
            },
            false,
        )
    }
    pub fn update_mode(ctx: &mut ctx::Context, auth: Auth) -> Self {
        let s = State::Account(AccountState::update(auth.name.clone(), auth.country));
        Self::make(ctx, s, auth, true)
    }
    pub fn email_mode(ctx: &mut ctx::Context, auth: Auth, logged_in: bool) -> Self {
        Self::make(ctx, State::email(), auth, logged_in)
    }
    pub fn link_entry_mode(ctx: &mut ctx::Context, auth: Auth) -> Self {
        Self::make(ctx, State::link_entry(), auth, false)
    }
    pub fn link_mode(ctx: &mut ctx::Context, auth: Auth) -> Self {
        Self::make(
            ctx,
            State::Link {
                code: None,
                expires: 0.0,
            },
            auth,
            true,
        )
    }
    pub fn devices_mode(ctx: &mut ctx::Context, auth: Auth) -> Self {
        Self::make(ctx, State::Devices { list: None }, auth, true)
    }

    fn back(&mut self, ctx: &mut ctx::Context) -> Transition {
        if self.logged_in {
            to_menu(
                ctx,
                self.auth.name.clone(),
                self.auth.country,
                self.auth.email.clone(),
            )
        } else {
            self.state = State::Account(AccountState::create(self.auth.country));
            Transition::None
        }
    }
}

impl Scene for AuthScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32, _alpha: f32) -> Transition {
        self.clock += dt;

        if !self.started {
            self.started = true;
            match &self.state {
                State::Link { .. } => {
                    let _ = ctx.net.send(&ClientMsg::MakeLinkCode);
                }
                State::Devices { .. } => {
                    let _ = ctx.net.send(&ClientMsg::ListDevices);
                }
                _ => {}
            }
        }

        let mouse = mouse_canvas(&ctx.rl);
        let clicked = ctx
            .rl
            .is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let back_hit = clicked && hit(SECONDARY_BTN, mouse);
        let mut back = false;
        let mut guest = false;
        let mut next: Option<State> = None;

        match &mut self.state {
            State::Idle => {}

            State::Email { field, pending } => {
                let mut submit = false;

                if back_hit {
                    back = true;
                } else if !*pending && single_update(&mut ctx.rl, field) {
                    submit = true;
                }

                if clicked && hit(PRIMARY_BTN, mouse) {
                    submit = true;
                }

                if submit {
                    if valid_email(&field.value) {
                        let _ = ctx.net.send(&ClientMsg::RequestCode {
                            email: field.value.trim().into(),
                        });
                        field.error = None;
                        *pending = true;
                    } else {
                        field.error = Some("enter a valid email");
                    }
                }
            }

            State::Code {
                email,
                field,
                resend_at,
                pending,
            } => {
                let mut submit = false;

                if back_hit {
                    back = true;
                } else if clicked && hit(PRIMARY_BTN, mouse) {
                    submit = true;
                } else if clicked && !*pending && self.clock >= *resend_at && hit(RESEND, mouse) {
                    let _ = ctx.net.send(&ClientMsg::RequestCode {
                        email: email.clone(),
                    });
                    *resend_at = self.clock + 60.0;
                    field.error = None;
                } else if !*pending && single_update(&mut ctx.rl, field) {
                    submit = true;
                }

                if submit {
                    if field.value.len() == 6 {
                        let _ = ctx.net.send(&ClientMsg::VerifyCode {
                            email: email.clone(),
                            code: field.value.clone(),
                        });
                        field.error = None;
                        *pending = true;
                    } else {
                        field.error = Some("6 digits");
                    }
                }
            }

            State::Account(acc) => {
                acc.shake = (acc.shake - dt).max(0.0);
                let mut submit = false;
                let mut secondary = false;

                if !acc.pending {
                    if clicked {
                        if hit(PRIMARY_BTN, mouse) {
                            submit = true;
                        } else if hit(SECONDARY_BTN, mouse) {
                            secondary = true;
                        } else if acc.mode == AccountMode::Create && hit(ALT_LEFT, mouse) {
                            next = Some(State::email());
                        } else if acc.mode == AccountMode::Create && hit(ALT_RIGHT, mouse) {
                            next = Some(State::link_entry());
                        }
                    }
                    let was = acc.focus;
                    acc.name.update(&mut ctx.rl, was.is(F_NAME));
                    acc.country
                        .update(&mut ctx.rl, was.is(F_COUNTRY), mouse, clicked);
                    acc.focus.update(
                        &ctx.rl,
                        mouse,
                        clicked,
                        &[acc.name.rect(), acc.country.rect()],
                    );
                    if enter_pressed(&ctx.rl) {
                        submit = true;
                    }
                }

                if submit {
                    match valid_name(&acc.name.value) {
                        Some(name) => {
                            acc.pending = true;
                            acc.name.error = None;
                            let _ = ctx.net.send(&acc.mode.message(name, acc.country.code()));
                        }
                        None => {
                            acc.name.error = Some(NAME_RULES);
                            acc.shake = SHAKE_TIME;
                            acc.focus.set(F_NAME);
                        }
                    }
                }
                if secondary {
                    if acc.mode == AccountMode::Create {
                        guest = true
                    } else {
                        back = true
                    }
                }
            }

            State::Link { code, expires } => {
                if back_hit {
                    back = true;
                } else if code.is_some() && self.clock >= *expires {
                    *code = None;
                    let _ = ctx.net.send(&ClientMsg::MakeLinkCode);
                }
            }

            State::LinkEntry { field, pending } => {
                let mut submit = false;

                if back_hit {
                    back = true;
                } else if !*pending && single_update(&mut ctx.rl, field) {
                    submit = true;
                }

                if clicked && hit(PRIMARY_BTN, mouse) {
                    submit = true;
                }

                if submit {
                    if field.value.len() == 8 {
                        let _ = ctx.net.send(&ClientMsg::RedeemLinkCode {
                            code: field.value.clone(),
                        });
                        field.error = None;
                        *pending = true;
                    } else {
                        field.error = Some("code has 8 characters");
                    }
                }
            }

            State::Devices { list } => {
                if back_hit {
                    back = true;
                } else if clicked {
                    if let Some(l) = list {
                        for (i, d) in l.iter().take(MAX_DEV).enumerate() {
                            let r: Rect = (
                                BOX_X + BOX_W - 44,
                                DEV_Y + i as i32 * DEV_ROW - 2,
                                44,
                                DEV_ROW,
                            );
                            if !d.current && hit(r, mouse) {
                                let _ = ctx.net.send(&ClientMsg::RevokeDevice { id: d.id.clone() });
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let Some(s) = next {
            self.state = s;
        }
        if guest {
            return to_menu(ctx, self.auth.name.clone(), self.auth.country, None);
        }
        if back {
            return self.back(ctx);
        }
        Transition::None
    }

    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let t = ctx.rl.get_time() as f32;
        let mouse = mouse_canvas(&ctx.rl);
        let pressed = ctx.rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(Color::new(15, 15, 15, 255));

        let mut pair: Option<(&'static str, &'static str)> = None;
        let mut back_btn = false;
        let mut alt = false;

        match &self.state {
            State::Idle => {
                let dots = ".".repeat(((t * 3.0) as usize % 3) + 1);
                centered(&mut tm, &format!("logging in{dots}"), 85, 10, DIM);
            }
            State::Email { field, pending } => {
                let sub = if self.logged_in {
                    "adds it to your account"
                } else {
                    "log in or sign up with a code"
                };
                header(&mut tm, "Email", Some(sub));
                field.draw(&mut tm, t, true, !*pending, 0.0);
                pair = Some(("NEXT", "BACK"))
            }
            State::Code {
                email,
                field,
                resend_at,
                pending,
            } => {
                header(&mut tm, "Enter code", Some(email));
                field.draw(&mut tm, t, true, !*pending, 0.0);
                let left = (*resend_at - self.clock).max(0.0).ceil() as i32;
                if left > 0 {
                    tm.draw_text(&format!("resend in {left}s"), RESEND.0, RESEND.1, 8, DIM);
                } else {
                    let c = if hit(RESEND, mouse) {
                        Color::WHITE
                    } else {
                        TEXT
                    };
                    tm.draw_text("resend code", RESEND.0, RESEND.1, 8, c);
                }
                pair = Some(("LOG IN", "BACK"))
            }
            State::Account(acc) => {
                header(&mut tm, acc.mode.title(), None);
                acc.name
                    .draw(&mut tm, t, acc.focus.is(F_NAME), !acc.pending, acc.shake);
                acc.country.draw(&mut tm, acc.focus.is(F_COUNTRY));
                pair = Some(acc.mode.button_labels(acc.pending));
                alt = acc.mode == AccountMode::Create && !acc.pending;
                if alt {
                    centered(&mut tm, "already have an account?", ALT_Y - 10, 8, DIM);
                }
            }
            State::Link { code, expires } => {
                header(
                    &mut tm,
                    "Link device",
                    Some("enter this code on your other device"),
                );
                match code {
                    Some(c) => {
                        centered(&mut tm, c, 70, 20, TEXT);
                        let left = (*expires - self.clock).max(0.0).ceil() as i32;
                        centered(&mut tm, &format!("{left}s"), 100, 8, DIM);
                    }
                    None => centered(&mut tm, "requesting...", 75, 10, DIM),
                }
                back_btn = true;
            }
            State::LinkEntry { field, pending } => {
                header(&mut tm, "Link device", None);
                field.draw(&mut tm, t, true, !*pending, 0.0);
                pair = Some(("LINK", "BACK"))
            }
            State::Devices { list } => {
                header(&mut tm, "Devices", Some("remove signs a device out"));
                match list {
                    None => centered(&mut tm, "loading...", 70, 10, DIM),
                    Some(l) => {
                        for (i, d) in l.iter().take(MAX_DEV).enumerate() {
                            let y = DEV_Y + i as i32 * DEV_ROW;
                            let label = if d.current {
                                "this device".to_string()
                            } else {
                                format!("device {}", &d.id[..d.id.len().min(4)])
                            };
                            tm.draw_text(&label, BOX_X, y, 8, TEXT);
                            tm.draw_text(&ago(d.last_seen), BOX_X + 76, y, 8, DIM);
                            if !d.current {
                                tm.draw_text("remove", BOX_X + BOX_W - 40, y, 8, RED);
                            }
                        }
                    }
                }
                back_btn = true;
            }
        }

        let mut btn = |r: Rect, label: &'static str| {
            self.button.draw(
                ctx.assets,
                &mut tm,
                r.0 as f32,
                r.1 as f32,
                r.2 as f32,
                mouse.into(),
                pressed,
                label,
                true,
                false,
            );
        };
        if let Some((p, s)) = pair {
            btn(PRIMARY_BTN, p);
            btn(SECONDARY_BTN, s);
        }
        if back_btn {
            btn(SECONDARY_BTN, "BACK");
        }
        if alt {
            btn(ALT_LEFT, "EMAIL LOGIN");
            btn(ALT_RIGHT, "LINK CODE");
        }
    }

    fn on_message(&mut self, ctx: &mut crate::ctx::Context, msg: &ServerMsg) -> Transition {
        match msg {
            ServerMsg::HelloOk {
                name,
                country,
                guest,
                email,
            } => {
                if !matches!(self.state, State::Idle) {
                    return Transition::None;
                }
                self.auth = Auth {
                    name: name.clone(),
                    country: *country,
                    email: email.clone(),
                };
                if *guest {
                    self.state = State::Account(AccountState::create(*country));
                } else {
                    self.logged_in = true;
                    return to_menu(ctx, name.clone(), *country, email.clone());
                }
            }

            ServerMsg::CodeSent => {
                let typed = if let State::Email { field, .. } = &self.state {
                    Some(field.value.trim().to_string())
                } else {
                    None
                };
                if let Some(email) = typed {
                    self.state = State::code(email, self.clock + 60.0);
                } else if let State::Code {
                    resend_at, field, ..
                } = &mut self.state
                {
                    *resend_at = self.clock + 60.0;
                    field.error = None;
                }
            }

            ServerMsg::CodeRejected(e) => {
                let mut expired = false;
                match &mut self.state {
                    State::Email { field, pending } => {
                        *pending = false;
                        field.error = Some(code_error_text(e, false));
                    }
                    State::Code { field, pending, .. } => {
                        *pending = false;
                        field.error = Some(code_error_text(e, false));
                        expired = matches!(e, CodeError::Expired);
                    }
                    State::LinkEntry { field, pending } => {
                        *pending = false;
                        field.error = Some(code_error_text(e, true));
                    }
                    _ => {}
                }
                if expired {
                    let mut s = State::email();
                    if let State::Email { field, .. } = &mut s {
                        field.error = Some("code expired, request a new one");
                    }
                    self.state = s;
                }
            }

            ServerMsg::NeedName => {
                if let State::Code { email, .. } = &self.state {
                    self.verified = Some(email.clone());
                }
                self.state = State::Account(AccountState::create(self.auth.country));
            }

            ServerMsg::Registered {
                token,
                name,
                country,
            } => {
                store::save("token", token);
                return to_menu(ctx, name.clone(), *country, self.verified.take());
            }
            ServerMsg::LoggedIn {
                token,
                name,
                country,
                email,
            } => {
                store::save("token", token);
                return to_menu(ctx, name.clone(), *country, email.clone());
            }
            ServerMsg::Updated { name, country } => {
                return to_menu(ctx, name.clone(), *country, self.auth.email.clone());
            }
            ServerMsg::EmailLinked { email } => {
                return to_menu(
                    ctx,
                    self.auth.name.clone(),
                    self.auth.country,
                    Some(email.clone()),
                );
            }
            ServerMsg::NameRejected(e) => {
                if let State::Account(acc) = &mut self.state {
                    acc.reject(match e {
                        NameError::Taken => "name already taken",
                        NameError::Invalid => NAME_RULES,
                        NameError::Busy => "try again later",
                    });
                }
            }

            ServerMsg::LinkCode { code } => {
                if let State::Link { code: c, expires } = &mut self.state {
                    *c = Some(code.clone());
                    *expires = self.clock + 300.0;
                }
            }
            ServerMsg::Devices { list } => {
                if let State::Devices { list: l } = &mut self.state {
                    *l = Some(list.clone());
                }
            }

            ServerMsg::LoggedOut => {
                store::save("token", "");
                let _ = ctx.net.reconnect();
                self.state = State::Idle;
                self.logged_in = false;
                self.verified = None;
            }
            _ => {}
        }
        Transition::None
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
