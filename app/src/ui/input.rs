use raylib::{
    RaylibHandle,
    consts::KeyboardKey,
    drawing::{RaylibDraw, RaylibTextureMode},
    ffi::Color,
};

type Tm<'a, 'b> = RaylibTextureMode<'a, 'b, RaylibHandle>;

pub const CANVAS_W: i32 = 320;
pub const CANVAS_H: i32 = 180;

pub const BOX_W: i32 = 200;
pub const BOX_H: i32 = 22;
pub const BOX_X: i32 = (CANVAS_W - BOX_W) / 2;

pub const CELL: i32 = 32;
pub const STRIP_X: i32 = BOX_X + (BOX_W - CELL * 5) / 2;

pub const SHAKE_TIME: f32 = 0.35;

pub type Rect = (i32, i32, i32, i32);

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

pub const CARD: Color = rgb(0, 0, 0);
pub const BORDER: Color = rgb(255, 255, 255);
pub const TEXT: Color = rgb(228, 232, 242);
pub const DIM: Color = rgb(170, 170, 170);
pub const DARK: Color = rgb(22, 18, 6);
pub const RED: Color = rgb(240, 90, 90);

pub fn mouse_canvas(rl: &RaylibHandle) -> (f32, f32) {
    let m = rl.get_mouse_position();
    (
        m.x * CANVAS_W as f32 / rl.get_screen_width().max(1) as f32,
        m.y * CANVAS_H as f32 / rl.get_screen_height().max(1) as f32,
    )
}

pub fn hit(r: Rect, p: (f32, f32)) -> bool {
    p.0 >= r.0 as f32 && p.0 < (r.0 + r.2) as f32 && p.1 >= r.1 as f32 && p.1 < (r.1 + r.3) as f32
}

pub fn enter_pressed(rl: &RaylibHandle) -> bool {
    rl.is_key_pressed(KeyboardKey::KEY_ENTER) || rl.is_key_pressed(KeyboardKey::KEY_KP_ENTER)
}

pub fn centered(tm: &mut Tm<'_, '_>, text: &str, y: i32, size: i32, color: Color) {
    let w = tm.measure_text(text, size);
    tm.draw_text(text, (CANVAS_W - w) / 2, y, size, color);
}

pub fn header(tm: &mut Tm<'_, '_>, title: &str, subtitle: Option<&str>) {
    tm.draw_text(title, BOX_X, 22, 8, Color::WHITE);
    if let Some(s) = subtitle {
        tm.draw_text(s, BOX_X, 34, 8, DIM);
    }
    tm.draw_rectangle(BOX_X, 50, BOX_W, 1, BORDER);
}

fn shake_offset(t: f32, shake: f32) -> i32 {
    if shake > 0.0 {
        ((t * 70.0).sin() * 4.0 * (shake / SHAKE_TIME)) as i32
    } else {
        0
    }
}

pub fn frame(tm: &mut Tm<'_, '_>, x: i32, y: i32, focused: bool, color: Option<Color>) {
    tm.draw_rectangle(x, y, BOX_W, BOX_H, CARD);
    let border = color.unwrap_or(if focused { Color::WHITE } else { BORDER });
    tm.draw_rectangle_lines(x, y, BOX_W, BOX_H, border);
}

#[derive(Default, Clone, Copy)]
pub struct Focus {
    pub idx: usize,
}

impl Focus {
    pub fn set(&mut self, i: usize) {
        self.idx = i;
    }

    pub fn is(&self, i: usize) -> bool {
        self.idx == i
    }

    pub fn update(&mut self, rl: &RaylibHandle, mouse: (f32, f32), clicked: bool, rects: &[Rect]) {
        let n = rects.len();
        if n == 0 {
            return;
        }

        if clicked {
            if let Some(i) = rects.iter().position(|r| hit(*r, mouse)) {
                self.idx = i;
            }
        }

        let shift = rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT)
            || rl.is_key_down(KeyboardKey::KEY_RIGHT_SHIFT);
        let next = (rl.is_key_pressed(KeyboardKey::KEY_TAB) && !shift)
            || rl.is_key_pressed(KeyboardKey::KEY_DOWN);
        let prev = (rl.is_key_pressed(KeyboardKey::KEY_TAB) && shift)
            || rl.is_key_pressed(KeyboardKey::KEY_UP);

        if next {
            self.idx = (self.idx + 1) % n;
        }
        if prev {
            self.idx = (self.idx + n - 1) % n;
        }
        self.idx %= n;
    }
}

pub struct TextField {
    pub label: &'static str,
    pub placeholder: String,
    pub value: String,
    pub max_len: usize,
    pub filter: fn(char) -> bool,
    pub hint: Option<fn(&str) -> (String, bool)>,
    pub error: Option<&'static str>,
    pub y: i32,
}

impl TextField {
    pub fn new(label: &'static str, y: i32, max_len: usize, filter: fn(char) -> bool) -> Self {
        Self {
            label,
            placeholder: String::new(),
            value: String::new(),
            max_len,
            filter,
            hint: None,
            error: None,
            y,
        }
    }

    pub fn with_placeholder(mut self, s: impl Into<String>) -> Self {
        self.placeholder = s.into();
        self
    }
    pub fn with_value(mut self, s: impl Into<String>) -> Self {
        self.value = s.into();
        self
    }
    pub fn with_hint(mut self, h: fn(&str) -> (String, bool)) -> Self {
        self.hint = Some(h);
        self
    }

    pub fn rect(&self) -> Rect {
        (BOX_X, self.y, BOX_W, BOX_H)
    }

    pub fn update(&mut self, rl: &mut RaylibHandle, focused: bool) {
        if !focused {
            while rl.get_char_pressed().is_some() {}
            return;
        }

        while let Some(c) = rl.get_char_pressed() {
            if (self.filter)(c) && self.value.chars().count() < self.max_len {
                self.value.push(c);
                self.error = None;
            }
        }

        if rl.is_key_pressed(KeyboardKey::KEY_BACKSPACE)
            || rl.is_key_pressed_repeat(KeyboardKey::KEY_BACKSPACE)
        {
            if rl.is_key_down(KeyboardKey::KEY_LEFT_CONTROL) {
                self.value.clear();
            } else {
                self.value.pop();
            }
            self.error = None;
        }
    }

    pub fn draw(&self, tm: &mut Tm<'_, '_>, t: f32, focused: bool, active: bool, shake: f32) {
        let bx = BOX_X + shake_offset(t, shake);

        tm.draw_text(self.label, BOX_X, self.y - 10, 8, DIM);
        frame(tm, bx, self.y, focused, self.error.is_some().then_some(RED));

        let ty = self.y + (BOX_H - 10) / 2;

        if self.value.is_empty() {
            tm.draw_text(&self.placeholder, bx + 8, ty, 10, DIM);
        } else {
            tm.draw_text(&self.value, bx + 8, ty, 10, TEXT);
        }

        if focused && active && (t * 2.0) as i32 % 2 == 0 {
            let cx = bx + 8 + tm.measure_text(&self.value, 10) + 1;
            tm.draw_rectangle(cx, ty - 1, 1, 12, Color::WHITE);
        }

        let line_y = self.y + BOX_H + 3;

        match self.error {
            Some(e) => tm.draw_text(e, bx, line_y, 8, RED),
            None => {
                if let Some(h) = self.hint {
                    let (text, bad) = h(&self.value);
                    tm.draw_text(&text, bx, line_y, 8, if bad { RED } else { DIM });
                }
            }
        }

        let count = format!("{}/{}", self.value.chars().count(), self.max_len);
        let cw = tm.measure_text(&count, 8);
        tm.draw_text(&count, BOX_X + BOX_W - cw, line_y, 8, DIM);
    }
}

const COUNTRIES: &str = "FI";

fn country_count() -> usize {
    COUNTRIES.split_whitespace().count().max(1)
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

pub struct CountryPicker {
    pub label: &'static str,
    pub y: i32,
    idx: usize,
    buf: String,
}

impl CountryPicker {
    pub fn new(label: &'static str, y: i32, current: [char; 2]) -> Self {
        Self {
            label,
            y,
            idx: country_index(current),
            buf: String::new(),
        }
    }

    pub fn code(&self) -> [char; 2] {
        country_chars(self.idx)
    }

    pub fn rect(&self) -> Rect {
        (BOX_X, self.y, BOX_W, BOX_H)
    }

    pub fn update(
        &mut self,
        rl: &mut RaylibHandle,
        focused: bool,
        mouse: (f32, f32),
        clicked: bool,
    ) {
        if !focused {
            self.buf.clear();
            while rl.get_char_pressed().is_some() {}
            return;
        }

        let n = country_count();

        if clicked && hit(self.rect(), mouse) {
            let k = if mouse.0 < STRIP_X as f32 {
                -1
            } else if mouse.0 >= (STRIP_X + CELL * 5) as f32 {
                1
            } else {
                (mouse.0 as i32 - STRIP_X) / CELL - 2
            };
            self.idx = (self.idx as i32 + k).rem_euclid(n as i32) as usize;
            self.buf.clear();
        }

        if rl.is_key_pressed(KeyboardKey::KEY_RIGHT)
            || rl.is_key_pressed_repeat(KeyboardKey::KEY_RIGHT)
        {
            self.idx = (self.idx + 1) % n;
            self.buf.clear();
        }
        if rl.is_key_pressed(KeyboardKey::KEY_LEFT)
            || rl.is_key_pressed_repeat(KeyboardKey::KEY_LEFT)
        {
            self.idx = (self.idx + n - 1) % n;
            self.buf.clear();
        }

        while let Some(c) = rl.get_char_pressed() {
            if !c.is_ascii_alphabetic() {
                continue;
            }
            if self.buf.len() >= 2 {
                self.buf.clear();
            }
            self.buf.push(c.to_ascii_uppercase());
            if let Some(p) = COUNTRIES
                .split_whitespace()
                .position(|x| x.starts_with(self.buf.as_str()))
            {
                self.idx = p;
            }
            if self.buf.len() == 2 {
                self.buf.clear();
            }
        }
    }

    pub fn draw(&self, tm: &mut Tm<'_, '_>, focused: bool) {
        let n = country_count();

        tm.draw_text(self.label, BOX_X, self.y - 10, 8, DIM);
        frame(tm, BOX_X, self.y, focused, None);

        let cy = self.y + (BOX_H - 10) / 2;

        if focused {
            for k in -2i32..=2 {
                let i = (self.idx as i32 + k).rem_euclid(n as i32) as usize;
                let code = country_code(i);
                let x = STRIP_X + (k + 2) * CELL;
                let w = tm.measure_text(code, 10);

                if k == 0 {
                    tm.draw_rectangle(x, self.y + 3, CELL, BOX_H - 6, Color::WHITE);
                    tm.draw_text(code, x + (CELL - w) / 2, cy, 10, DARK);
                } else {
                    tm.draw_text(code, x + (CELL - w) / 2, cy, 10, DIM);
                }
            }

            tm.draw_text("<", BOX_X + 5, cy, 10, TEXT);
            tm.draw_text(">", BOX_X + BOX_W - 11, cy, 10, TEXT);
        } else {
            tm.draw_text(country_code(self.idx), BOX_X + 8, cy, 10, TEXT);
        }
    }
}
