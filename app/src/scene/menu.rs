use raylib::{
    drawing::{RaylibDraw, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, MouseButton, Vector2},
    texture::Texture2D,
};
use wrum_proto::{BoardKind, ClientMsg, RejectReason, ServerMsg};

use crate::{
    backdrop::Backdrop,
    scene::{
        Scene,
        auth::{Auth, AuthScene},
        game::{GameScene, format_time},
    },
    ui::Button,
};

const MAPS: &[&str] = &["assets/maps/untitled_bot.json"];

type R = (f32, f32, f32, f32);

const SCALE: f32 = 4.0;
const LOGO_POS: (i32, i32) = (16, 24);
const QUICK: R = (16., 59., 100., 0.);
const CREATE: R = (16., 75., 100., 0.);
const JOIN: R = (16., 91., 100., 0.);
const EDIT: R = (268., 41., 36., 15.);

const LEADERBOARD: R = (16., 120., 100., 0.);
const SETTINGS: R = (16., 136., 100., 0.);

const JOIN2: R = (109., 83., 50., 0.);
const CANCEL: R = (161., 83., 50., 0.);

const LCANCEL: R = (16., 16., 55., 0.);
const TOTAL: R = (190., 16., 58., 0.);
const LAP: R = (250., 16., 58., 0.);

const GOLD: Color = Color::new(255, 215, 64, 255);
const SILVER: Color = Color::new(200, 205, 215, 255);
const BRONZE: Color = Color::new(205, 127, 80, 255);
const DIM: Color = rgb(122, 132, 154);

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

const RED: Color = rgb(240, 90, 90);

type Board = (Vec<(String, f32, [char; 2])>, Option<(u32, f32)>);

fn idx(k: BoardKind) -> usize {
    match k {
        BoardKind::Total => 0,
        BoardKind::Lap => 1,
    }
}

pub struct MenuScene {
    logo: Texture2D,
    status: String,
    join_open: bool,
    leaderboard_open: bool,
    code: String,
    auth: Auth,
    boards: [Option<Board>; 2],
    kind: BoardKind,
    mouse: (f32, f32),
    clock: f32,
    want_fetch: bool,
    next_fetch: f32,
    refresh_at: f32,
    backdrop: Backdrop,
    button: Button,
}

impl MenuScene {
    pub fn new(status: &str, ctx: &mut crate::ctx::Context, auth: Auth) -> Self {
        let pick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0)
            % MAPS.len();

        Self {
            logo: ctx.rl.load_texture(ctx.thread, "assets/logo.png").unwrap(),
            status: status.into(),
            join_open: false,
            leaderboard_open: false,
            code: String::new(),
            auth,
            boards: [None, None],
            kind: BoardKind::Total,
            mouse: (-1.0, -1.0),
            clock: 0.0,
            want_fetch: false,
            next_fetch: 0.0,
            refresh_at: 20.0,
            backdrop: Backdrop::new(ctx, MAPS[pick]),
            button: Button::new(ctx),
        }
    }

    fn is_guest(&self) -> bool {
        self.auth.name.starts_with("guest-")
    }
    fn can_join(&self) -> bool {
        self.code.len() == 6
    }
    fn join_code(&mut self, ctx: &mut crate::ctx::Context) {
        self.join_open = false;
        let _ = ctx.net.send(&ClientMsg::JoinLobby {
            code: self.code.clone(),
        });
        self.code = String::new();
    }
}

impl Scene for MenuScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32, _alpha: f32) -> super::Transition {
        self.clock += dt;
        self.backdrop.update(ctx, dt);
        let m = ctx.rl.get_mouse_position();
        self.mouse = (m.x / SCALE, m.y / SCALE);

        if ctx
            .rl
            .is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
        {
            let mp = self.mouse.into();
            if !self.join_open && !self.leaderboard_open {
                if self.button.inside(QUICK.0, QUICK.1, QUICK.2, mp) {
                    let _ = ctx.net.send(&ClientMsg::JoinPublic);
                } else if self.button.inside(CREATE.0, CREATE.1, CREATE.2, mp) {
                    let _ = ctx.net.send(&ClientMsg::CreateLobby);
                } else if self.button.inside(JOIN.0, JOIN.1, JOIN.2, mp) {
                    self.join_open = true;
                } else if self
                    .button
                    .inside(LEADERBOARD.0, LEADERBOARD.1, LEADERBOARD.2, mp)
                {
                    self.leaderboard_open = true;
                    self.want_fetch = true;
                } else if self.button.inside(SETTINGS.0, SETTINGS.1, SETTINGS.2, mp) {
                    // todo: settings scene
                } else if self.button.inside(EDIT.0, EDIT.1, EDIT.2, mp) && !self.is_guest() {
                    return super::Transition::Switch(Box::new(AuthScene::update_mode(
                        ctx,
                        self.auth.name.clone(),
                        self.auth.country,
                    )));
                }
            } else if self.join_open {
                if self.button.inside(JOIN2.0, JOIN2.1, JOIN2.2, mp) && self.can_join() {
                    self.join_code(ctx);
                } else if self.button.inside(CANCEL.0, CANCEL.1, CANCEL.2, mp) {
                    self.join_open = false;
                    self.code = String::new();
                }
            } else if self.leaderboard_open {
                if self.button.inside(LCANCEL.0, LCANCEL.1, LCANCEL.2, mp) {
                    self.leaderboard_open = false;
                    self.want_fetch = true;
                } else if self.button.inside(TOTAL.0, TOTAL.1, TOTAL.2, mp) {
                    self.kind = BoardKind::Total;
                    self.want_fetch = true;
                } else if self.button.inside(LAP.0, LAP.1, LAP.2, mp) {
                    self.kind = BoardKind::Lap;
                    self.want_fetch = true;
                }
            }
        }

        while let Some(c) = ctx.rl.get_char_pressed() {
            if c.is_ascii_digit() && self.code.len() < 6 {
                self.code.push(c);
            }
        }
        if ctx.rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE) {
            self.code.pop();
        }
        if ctx.rl.is_key_pressed(KeyboardKey::KEY_ENTER) && self.can_join() {
            self.join_code(ctx);
        }

        if self.clock >= self.refresh_at {
            self.want_fetch = true;
            self.refresh_at = self.clock + 20.0;
        }
        if self.want_fetch && self.clock >= self.next_fetch {
            let _ = ctx.net.send(&ClientMsg::GetLeaderboard { kind: self.kind });
            self.want_fetch = false;
            self.next_fetch = self.clock + 1.2;
        }
        super::Transition::None
    }

    fn draw(&mut self, ctx: &mut crate::ctx::Context) {
        let pressed = ctx.rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);

        self.backdrop.prepare(ctx);
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        self.backdrop.draw(ctx.assets, &mut tm);
        let mp = self.mouse.into();

        if !self.leaderboard_open {
            tm.draw_texture(&self.logo, LOGO_POS.0, LOGO_POS.1, Color::WHITE);
            if !self.status.is_empty() {
                tm.draw_text(&self.status, 16, 45, 8, RED);
            }

            {
                let mp = if self.join_open { Vector2::zero() } else { mp };

                self.button.draw(
                    ctx.assets,
                    &mut tm,
                    QUICK.0,
                    QUICK.1,
                    QUICK.2,
                    mp,
                    pressed,
                    "Quickplay",
                    false,
                    false,
                );
                self.button.draw(
                    ctx.assets,
                    &mut tm,
                    CREATE.0,
                    CREATE.1,
                    CREATE.2,
                    mp,
                    pressed,
                    "Create private",
                    false,
                    false,
                );
                self.button.draw(
                    ctx.assets,
                    &mut tm,
                    JOIN.0,
                    JOIN.1,
                    JOIN.2,
                    mp,
                    pressed,
                    "Join private",
                    false,
                    false,
                );

                self.button.draw(
                    ctx.assets,
                    &mut tm,
                    LEADERBOARD.0,
                    LEADERBOARD.1,
                    LEADERBOARD.2,
                    mp,
                    pressed,
                    "Leaderboard",
                    false,
                    false,
                );
                self.button.draw(
                    ctx.assets, &mut tm, SETTINGS.0, SETTINGS.1, SETTINGS.2, mp, pressed,
                    "Settings", false, false,
                );

                let w = tm.measure_text(&self.auth.name, 8);
                tm.draw_text(&self.auth.name, 300 - w, 31, 8, Color::WHITE);
                self.button.draw(
                    ctx.assets, &mut tm, EDIT.0, EDIT.1, EDIT.2, mp, pressed, "edit", true, false,
                );
            }

            if self.join_open {
                tm.draw_rectangle(0, 0, 320, 180, Color::new(0, 0, 0, 150));

                let w = tm.measure_text("Enter 6 digit code", 8);
                tm.draw_text("Enter 6 digit code", 160 - w / 2, 50, 8, Color::WHITE);
                let w = tm.measure_text(&self.code, 20);
                tm.draw_text(&self.code, 160 - w / 2, 60, 20, Color::WHITE);

                self.button.draw(
                    ctx.assets, &mut tm, JOIN2.0, JOIN2.1, JOIN2.2, mp, pressed, "Join", true,
                    false,
                );
                self.button.draw(
                    ctx.assets, &mut tm, CANCEL.0, CANCEL.1, CANCEL.2, mp, pressed, "Cancel", true,
                    false,
                );
            }
        } else {
            self.button.draw(
                ctx.assets, &mut tm, LCANCEL.0, LCANCEL.1, LCANCEL.2, mp, pressed, "Go back", true,
                false,
            );
            self.button.draw(
                ctx.assets,
                &mut tm,
                TOTAL.0,
                TOTAL.1,
                TOTAL.2,
                mp,
                pressed,
                "Total",
                true,
                self.kind == BoardKind::Total,
            );
            self.button.draw(
                ctx.assets,
                &mut tm,
                LAP.0,
                LAP.1,
                LAP.2,
                mp,
                pressed,
                "Best lap",
                true,
                self.kind == BoardKind::Lap,
            );

            let board = self.boards[idx(self.kind)].as_ref();
            match board {
                None => {
                    let w = tm.measure_text("Loading...", 8);
                    tm.draw_text("Loading...", 160 - w / 2, 86, 8, Color::WHITE);
                }
                Some((rows, _)) if rows.is_empty() => {
                    let w = tm.measure_text("No times yet", 8);
                    tm.draw_text("No times yet", 160 - w / 2, 86, 8, Color::WHITE);
                }
                Some((rows, _)) => {
                    for (i, (name, t, country)) in rows.iter().take(10).enumerate() {
                        let y = 49 + i as i32 * 11;
                        let me = name.eq_ignore_ascii_case(&self.auth.name);
                        if i % 2 == 0 {
                            tm.draw_rectangle(16, y, 300, 11, Color::new(0, 0, 0, 180));
                        } else {
                            tm.draw_rectangle(16, y, 300, 11, Color::new(0, 0, 0, 120));
                        }
                        let rc = match i {
                            0 => GOLD,
                            1 => SILVER,
                            2 => BRONZE,
                            _ => DIM,
                        };

                        tm.draw_text(&format!("{}", i + 1), 21, y + 1, 8, rc);
                        tm.draw_text(&format!("{}{}", country[0], country[1]), 56, y + 1, 8, DIM);
                        tm.draw_text(&name, 73, y + 1, 8, if me { RED } else { Color::WHITE });
                        let ts = format_time(*t);
                        let w = tm.measure_text(&ts, 8);
                        tm.draw_text(
                            &ts,
                            296 - w,
                            y + 1,
                            8,
                            if i == 0 { GOLD } else { Color::WHITE },
                        );
                    }
                }
            }

            tm.draw_line(16, 164, 300, 164, Color::WHITE);
            match board.and_then(|b| b.1) {
                Some((rank, t)) => {
                    tm.draw_text(&format!("You  #{rank}"), 21, 167, 8, Color::WHITE);
                    let ts = format_time(t);
                    let w = tm.measure_text(&ts, 8);
                    tm.draw_text(&ts, 296 - w, 167, 8, Color::WHITE);
                }
                None if self.is_guest() => {
                    tm.draw_text("Sign up to get ranked", 21, 167, 8, Color::WHITE)
                }
                None => tm.draw_text("Finish a race to get ranked", 21, 167, 8, Color::WHITE),
            }
        }
    }

    fn on_message(&mut self, ctx: &mut crate::ctx::Context, msg: &ServerMsg) -> super::Transition {
        match msg {
            ServerMsg::Welcome {
                slot,
                players,
                code,
                private,
                ..
            } => super::Transition::Switch(Box::new(GameScene::new(
                ctx, *slot, players, code, *private,
            ))),
            ServerMsg::Leaderboard { kind, rows, mine } => {
                self.boards[idx(*kind)] = Some((rows.clone(), *mine));
                super::Transition::None
            }
            ServerMsg::Reject(reason) => {
                match reason {
                    RejectReason::LobbyFull => self.status = "lobby full".into(),
                    RejectReason::LobbyGone => self.status = "lobby gone".into(),
                    RejectReason::LobbyNotFound => self.status = "lobby not found".into(),
                    RejectReason::RaceStarted => self.status = "race already started".into(),
                    _ => {}
                }
                super::Transition::None
            }
            _ => super::Transition::None,
        }
    }

    fn blocks_draw(&self) -> bool {
        true
    }
    fn blocks_update(&self) -> bool {
        true
    }
}
