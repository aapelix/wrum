use raylib::{
    RaylibHandle,
    drawing::{RaylibDraw, RaylibTextureMode, RaylibTextureModeExt},
    ffi::{Color, KeyboardKey, MouseButton},
    texture::Texture2D,
};
use wrum_proto::{BoardKind, ClientMsg, ServerMsg};

use crate::scene::{
    Scene,
    auth::{Auth, AuthScene},
    game::{GameScene, format_time},
};

type R = (i32, i32, i32, i32);

const SCALE: f32 = 4.0;
const LOGO_POS: (i32, i32) = (16, 10);
const QUICK: R = (16, 48, 120, 20);
const CREATE: R = (16, 72, 120, 20);
const FIELD: R = (16, 96, 70, 20);
const JOIN: R = (90, 95, 46, 20);
const PANEL: R = (148, 12, 160, 156);
const TAB_TOTAL: R = (156, 30, 44, 13);
const TAB_LAP: R = (204, 30, 52, 13);
const EDIT: R = (110, 137, 26, 15);

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
const ME: Color = rgb(42, 60, 104);
const GOLD: Color = rgb(255, 205, 70);
const SILVER: Color = rgb(205, 212, 224);
const BRONZE: Color = rgb(216, 140, 84);
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
    code: String,
    auth: Auth,
    boards: [Option<Board>; 2],
    kind: BoardKind,
    mouse: (f32, f32),
    clock: f32,
    want_fetch: bool,
    next_fetch: f32,
    refresh_at: f32,
}

impl MenuScene {
    pub fn new(status: &str, ctx: &mut crate::ctx::Context, auth: Auth) -> Self {
        Self {
            logo: ctx.rl.load_texture(ctx.thread, "assets/logo.png").unwrap(),
            status: status.into(),
            code: String::new(),
            auth,
            boards: [None, None],
            kind: BoardKind::Total,
            mouse: (-1.0, -1.0),
            clock: 0.0,
            want_fetch: true,
            next_fetch: 0.0,
            refresh_at: 20.0,
        }
    }

    fn is_guest(&self) -> bool {
        self.auth.name.starts_with("guest-")
    }
    fn can_join(&self) -> bool {
        self.code.len() == 6
    }
    fn join_code(&self, ctx: &mut crate::ctx::Context) {
        let _ = ctx.net.send(&ClientMsg::JoinLobby {
            code: self.code.clone(),
        });
    }
}

impl Scene for MenuScene {
    fn update(&mut self, ctx: &mut crate::ctx::Context, dt: f32, _alpha: f32) -> super::Transition {
        self.clock += dt;
        let m = ctx.rl.get_mouse_position();
        self.mouse = (m.x / SCALE, m.y / SCALE);

        if ctx
            .rl
            .is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
        {
            let mp = self.mouse;
            if hit(QUICK, mp) {
                let _ = ctx.net.send(&ClientMsg::JoinPublic);
            } else if hit(CREATE, mp) {
                let _ = ctx.net.send(&ClientMsg::CreateLobby);
            } else if hit(JOIN, mp) && self.can_join() {
                self.join_code(ctx);
            } else if hit(TAB_TOTAL, mp) && self.kind != BoardKind::Total {
                self.kind = BoardKind::Total;
                self.want_fetch = true;
            } else if hit(TAB_LAP, mp) && self.kind != BoardKind::Lap {
                self.kind = BoardKind::Lap;
                self.want_fetch = true;
            } else if hit(EDIT, mp) && !self.is_guest() {
                return super::Transition::Switch(Box::new(AuthScene::update_mode(
                    self.auth.name.clone(),
                    self.auth.country,
                )));
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
        let mut tm = ctx.rl.begin_texture_mode(&ctx.thread, &mut ctx.canvas);
        tm.clear_background(BG);
        let mp = self.mouse;

        tm.draw_texture(&self.logo, LOGO_POS.0, LOGO_POS.1, Color::WHITE);
        if !self.status.is_empty() {
            tm.draw_text(&self.status, 16, 37, 8, RED);
        }

        button(&mut tm, QUICK, "Quickplay", hit(QUICK, mp), true, true);
        button(
            &mut tm,
            CREATE,
            "Create private",
            hit(CREATE, mp),
            false,
            true,
        );

        fill(&mut tm, FIELD, CARD);
        frame(
            &mut tm,
            FIELD,
            if self.code.is_empty() { BORDER } else { ACCENT },
        );
        if self.code.is_empty() {
            tm.draw_text("6-digit code", FIELD.0 + 6, FIELD.1 + 6, 8, DIM);
        } else {
            tm.draw_text(&self.code, FIELD.0 + 6, FIELD.1 + 5, 10, TEXT);
            if (self.clock * 2.0) as i32 % 2 == 0 && self.code.len() < 6 {
                let w = tm.measure_text(&self.code, 10);
                tm.draw_rectangle(FIELD.0 + 7 + w, FIELD.1 + 4, 1, 12, ACCENT);
            }
        }
        button(&mut tm, JOIN, "Join", hit(JOIN, mp), true, self.can_join());

        tm.draw_text("PLAYING AS", 16, 138, 8, DIM);
        let who = fit(&tm, &self.auth.name, 118, 10);
        tm.draw_text(&who, 16, 148, 10, TEXT);
        if self.is_guest() {
            tm.draw_text("Guest: not ranked", 16, 161, 8, ACCENT);
        }
        button(
            &mut tm,
            EDIT,
            "edit",
            hit(EDIT, mp),
            false,
            !self.is_guest(),
        );

        fill(&mut tm, PANEL, CARD);
        frame(&mut tm, PANEL, BORDER);
        tm.draw_text("LEADERBOARD", PANEL.0 + 8, PANEL.1 + 5, 8, DIM);
        tab(
            &mut tm,
            TAB_TOTAL,
            "Total",
            self.kind == BoardKind::Total,
            hit(TAB_TOTAL, mp),
        );
        tab(
            &mut tm,
            TAB_LAP,
            "Best lap",
            self.kind == BoardKind::Lap,
            hit(TAB_LAP, mp),
        );
        tm.draw_line(PANEL.0 + 4, 46, PANEL.0 + PANEL.2 - 5, 46, BORDER);

        let right = PANEL.0 + PANEL.2 - 8;
        let board = self.boards[idx(self.kind)].as_ref();
        match board {
            None => centered(&mut tm, "Loading...", 98, DIM),
            Some((rows, _)) if rows.is_empty() => {
                centered(&mut tm, "No times yet", 92, DIM);
                centered(&mut tm, "Finish a race!", 104, DIM);
            }
            Some((rows, _)) => {
                for (i, (name, t, country)) in rows.iter().take(10).enumerate() {
                    let y = 49 + i as i32 * 10;
                    let me = name.eq_ignore_ascii_case(&self.auth.name);
                    if me {
                        tm.draw_rectangle(PANEL.0 + 3, y, PANEL.2 - 6, 10, ME);
                    } else if i % 2 == 0 {
                        tm.draw_rectangle(PANEL.0 + 3, y, PANEL.2 - 6, 10, STRIPE);
                    }
                    let rc = match i {
                        0 => GOLD,
                        1 => SILVER,
                        2 => BRONZE,
                        _ => DIM,
                    };
                    tm.draw_text(&format!("{}", i + 1), PANEL.0 + 8, y + 1, 8, rc);
                    tm.draw_text(
                        &format!("{}{}", country[0], country[1]),
                        PANEL.0 + 26,
                        y + 1,
                        8,
                        DIM,
                    );
                    let n = fit(&tm, name, 62, 8);
                    tm.draw_text(
                        &n,
                        PANEL.0 + 44,
                        y + 1,
                        8,
                        if me { ACCENT_HI } else { TEXT },
                    );
                    let ts = format_time(*t);
                    let w = tm.measure_text(&ts, 8);
                    tm.draw_text(&ts, right - w, y + 1, 8, if i == 0 { GOLD } else { TEXT });
                }
            }
        }

        let fy = PANEL.1 + PANEL.3 - 14;
        tm.draw_line(PANEL.0 + 4, fy - 3, PANEL.0 + PANEL.2 - 5, fy - 3, BORDER);
        match board.and_then(|b| b.1) {
            Some((rank, t)) => {
                tm.draw_text(&format!("You  #{rank}"), PANEL.0 + 8, fy, 8, ACCENT);
                let ts = format_time(t);
                let w = tm.measure_text(&ts, 8);
                tm.draw_text(&ts, right - w, fy, 8, ACCENT);
            }
            None if self.is_guest() => {
                tm.draw_text("Sign up to get ranked", PANEL.0 + 8, fy, 8, DIM)
            }
            None => tm.draw_text("Finish a race to get ranked", PANEL.0 + 8, fy, 8, DIM),
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

// ---------------------------------------------------------------- helpers

fn hit(r: R, m: (f32, f32)) -> bool {
    m.0 >= r.0 as f32 && m.0 < (r.0 + r.2) as f32 && m.1 >= r.1 as f32 && m.1 < (r.1 + r.3) as f32
}
fn fill(tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, r: R, c: Color) {
    tm.draw_rectangle(r.0, r.1, r.2, r.3, c);
}
fn frame(tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, r: R, c: Color) {
    tm.draw_rectangle_lines(r.0, r.1, r.2, r.3, c);
}
fn centered(tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>, s: &str, y: i32, c: Color) {
    let w = tm.measure_text(s, 8);
    tm.draw_text(s, PANEL.0 + (PANEL.2 - w) / 2, y, 8, c);
}
fn fit(tm: &RaylibTextureMode<'_, '_, RaylibHandle>, s: &str, max: i32, size: i32) -> String {
    if tm.measure_text(s, size) <= max {
        return s.to_string();
    }
    let mut v: Vec<char> = s.chars().collect();
    while !v.is_empty() {
        v.pop();
        let t: String = v.iter().collect::<String>() + "..";
        if tm.measure_text(&t, size) <= max {
            return t;
        }
    }
    String::new()
}
fn button(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    r: R,
    label: &str,
    hover: bool,
    primary: bool,
    enabled: bool,
) {
    let (face, edge, ink) = match (primary, enabled) {
        (_, false) => (BTN, BORDER, DIM),
        (true, true) => (if hover { ACCENT_HI } else { ACCENT }, ACCENT_LO, DARK),
        (false, true) => (if hover { BTN_HI } else { BTN }, BORDER, TEXT),
    };
    let lift = if hover && enabled { 0 } else { 1 };
    tm.draw_rectangle(r.0, r.1 + 2, r.2, r.3 - 1, edge);
    tm.draw_rectangle(r.0, r.1 + 2 - lift * 2 + lift, r.2, r.3 - 2, face);
    let w = tm.measure_text(label, 8);
    tm.draw_text(
        label,
        r.0 + (r.2 - w) / 2,
        r.1 + (r.3 - 8) / 2 + 1 - lift + 1,
        8,
        ink,
    );
}
fn tab(
    tm: &mut RaylibTextureMode<'_, '_, RaylibHandle>,
    r: R,
    label: &str,
    active: bool,
    hover: bool,
) {
    if active {
        fill(tm, r, ACCENT);
    } else if hover {
        fill(tm, r, BTN_HI);
    } else {
        fill(tm, r, BTN);
    }
    let w = tm.measure_text(label, 8);
    tm.draw_text(
        label,
        r.0 + (r.2 - w) / 2,
        r.1 + 3,
        8,
        if active { DARK } else { TEXT },
    );
}
