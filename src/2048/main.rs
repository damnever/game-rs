extern crate termion;
extern crate rand;

use std::io::{self, Read, Write};
use std::collections::{HashSet, HashMap};

use termion::{clear, cursor, style, color};
use termion::raw::IntoRawMode;
use termion::input::TermRead;
use termion::event::Key;
use rand::random;


struct Game<R, W: Write> {
    score: u32,
    grid: [u32; 16usize],
    bgs: HashMap<u32, color::Bg<color::Rgb>>,
    stdin: R,
    stdout: W,
}

fn init<R: Read, W: Write>(stdin: R, mut stdout: W) {
    write!(stdout, "{}", clear::All).unwrap();

    let mut bgs: HashMap<u32, color::Bg<color::Rgb>> = HashMap::with_capacity(17);
    // Ref: http://www.rapidtables.com/web/color/RGB_Color.htm
    // should cover all..
    let colors = [
        color::Bg(color::Rgb(224, 224, 224)), color::Bg(color::Rgb(255, 229, 204)),
        color::Bg(color::Rgb(255, 153, 153)), color::Bg(color::Rgb(204, 255, 209)),
        color::Bg(color::Rgb(204, 255, 255)), color::Bg(color::Rgb(204, 229, 255)),
        color::Bg(color::Rgb(204, 255, 153)), color::Bg(color::Rgb(204, 153, 255)),
        color::Bg(color::Rgb(255, 153, 255)), color::Bg(color::Rgb(255, 153, 51)),
        color::Bg(color::Rgb(255, 255, 51)), color::Bg(color::Rgb(255, 178, 102)),
        color::Bg(color::Rgb(178, 255, 102)), color::Bg(color::Rgb(102, 255, 178)),
        color::Bg(color::Rgb(102, 178, 255)), color::Bg(color::Rgb(102, 102, 255)),
        color::Bg(color::Rgb(255, 0, 0)),
    ];
    bgs.insert(0u32, colors[0]);
    for i in 1..17 {
        bgs.insert(1u32<<i as u32, colors[i]);
    }

    let mut game = Game {
        score: 0,
        grid: [0u32; 16usize],
        bgs: bgs,
        stdin: stdin.keys(),
        stdout: stdout,
    };

    game.reset();
    game.start();
}

impl <R, W: Write> Drop for Game<R, W> {
    fn drop(&mut self) {
        write!(self.stdout, "{}{}{}", clear::All, style::Reset,
               cursor::Goto(1, 1)).unwrap();
    }
}

macro_rules! continue_if_game_over {
    ($game_over:ident) => (
        if $game_over {
            continue;
        }
    )
}

impl <R: Iterator<Item=Result<Key, std::io::Error>>, W: Write> Game<R, W> {
    fn pos(&self, x: i32, y: i32) -> usize {
        (x * 4 + y) as usize
    }

    fn get_by_pos(&self, x: i32, y: i32) -> u32 {
        self.grid[self.pos(x, y)]
    }

    fn set_by_pos(&mut self, x: i32, y: i32, val: u32) {
        self.grid[self.pos(x, y)] = val;
    }

    fn start(&mut self) {
        let mut game_over = false;

        loop {
            let mut moved = false;
            let b = self.stdin.next().unwrap().unwrap();

            match b {
                Key::Up | Key::Char('w') | Key::Char('k') => {
                    continue_if_game_over!(game_over);
                    moved = self.up();
                },
                Key::Right | Key::Char('d') | Key::Char('l') => {
                    continue_if_game_over!(game_over);
                    moved = self.right();
                },
                Key::Down | Key::Char('s') | Key::Char('j') => {
                    continue_if_game_over!(game_over);
                    moved = self.down();
                },
                Key::Left | Key::Char('a') | Key::Char('h') => {
                    continue_if_game_over!(game_over);
                    moved = self.left();
                },
                Key::Char('r') => {
                    self.restart();
                    return;
                },
                Key::Esc | Key::Char('q') => return,
                _ => continue,
            }

            if !moved || game_over {
                continue;
            }
            let full_filled = self.fill_up();
            self.draw();
            if full_filled && self.game_over() {
                game_over = true;
                self.pop_menu();
            }
        }
    }

    fn reset(&mut self) {
        for i in 0..16usize {
            self.grid[i] = 0u32;
        }
        self.score = 0u32;
        let idx1 = random::<u8>() as usize % 16usize;
        self.grid[idx1] = 2u32;
        loop {
            let idx2 = random::<u8>() as usize % 16usize;
            if idx1 != idx2 {
                self.grid[idx2] = 2u32;
                break;
            }
        }
        self.draw();
    }

    fn restart(&mut self) {
        self.reset();
        self.start();
    }

    fn fill_up(&mut self) -> bool {
        let mut holes = HashSet::new();
        for i in 0..16usize {
            if self.grid[i] == 0u32 {
                holes.insert(i);
            }
        }

        let len = holes.len();
        if len == 0 {
            return true;
        }
        for idx in holes.iter() {  // use HashSet's random feature directly..
            // self.grid[*idx] = 1u32 << (*idx as u32 % 2u32 + 1u32);  // whatever...
            let mut threhold = 222u8;
            if len <= 4 {
                threhold = 127u8;
            }
            if random::<u8>() <= threhold {
                self.grid[*idx] = 2u32;
            } else {
                self.grid[*idx] = 4u32;
            }
            break;
        }

        (len-1) == 0
    }

    fn game_over(&mut self) -> bool {
        for x in 0..4 {
            for y in 1..4 {
                if self.get_by_pos(x, y-1) == self.get_by_pos(x, y) {
                    return false;
                }
            }
        }
        for y in 0..4 {
            for x in 1..4 {
                if self.get_by_pos(x-1, y) == self.get_by_pos(x, y) {
                    return false;
                }
            }
        }
        return true;
    }

    fn bg(&self, n: u32) -> color::Bg<color::Rgb> {
        match self.bgs.get(&n) {
            Some(bg) => *bg,
            None => color::Bg(color::Rgb(255, 255, 255)),
        }
    }

    fn draw(&mut self) {
        write!(self.stdout, "{}", cursor::Goto(1, 1)).unwrap();

        // header
        let header_bg = color::Bg(color::Rgb(128, 128, 128));
        write!(self.stdout, " {}{:31}{}\n\r", header_bg, " ", style::Reset).unwrap();
        let score = self.score.to_string();
        write!(self.stdout, " {}{} SCORE: {}{:space$}{}\n\r", header_bg, style::Bold, score,
               " ", style::Reset, space=23-score.len()).unwrap();
        write!(self.stdout, " {}{:31}{}\n\r", header_bg, " ", style::Reset).unwrap();
        write!(self.stdout, " {:32}\n\r", " ").unwrap();

        // body
        for x in 0..4i32 {
            let mut up_line: String = String::new();
            let mut mid_line: String = String::new();
            let mut down_line: String = String::new();
            for y in 0..4i32 {
                let val = self.get_by_pos(x, y);
                let sval = val.to_string();
                let bg = self.bg(val);

                up_line.push_str(format!(" {}{:7}{}", bg, " ", style::Reset).as_str());
                if val == 0u32 {
                    mid_line.push_str(format!(" {}{:7}{}", bg, " ", style::Reset).as_str());
                } else {
                    let pad = (7 - sval.len()) / 2;
                    mid_line.push_str(
                        format!(" {}{:lpad$}{}{}{}{}{:rpad$}{}", bg, " ", color::Fg(color::Black),
                                sval, style::Reset, bg, " ", style::Reset,
                                lpad=pad, rpad=7-sval.len()-pad).as_str());
                }
                down_line.push_str(format!(" {}{:7}{}", bg, " ", style::Reset).as_str());
            }
            up_line.push_str("\n\r");
            mid_line.push_str("\n\r");
            down_line.push_str("\n\r");
            self.stdout.write(up_line.as_bytes()).unwrap();
            self.stdout.write(mid_line.as_bytes()).unwrap();
            self.stdout.write(down_line.as_bytes()).unwrap();
            self.stdout.write(format!("{:32}\n\r", " ").as_bytes()).unwrap();
        }
        self.stdout.flush().unwrap();
    }

    fn pop_menu(&mut self) {
        let bg = color::Bg(color::Rgb(128, 128, 128));
        write!(self.stdout, "{}{}{}  GAME OVER!   {}\n\r",
               cursor::Goto(10, 4), bg, style::Bold, style::Reset).unwrap();
        write!(self.stdout, "{}{}  restart: r   {}\n\r",
               cursor::Goto(10, 5), bg, style::Reset).unwrap();
        write!(self.stdout, "{}{}  quit: ESC|q  {}\n\r",
               cursor::Goto(10, 6), bg, style::Reset).unwrap();
        write!(self.stdout, "{}{}               {}\n\r",
               cursor::Goto(10, 7), bg, style::Reset).unwrap();
    }

    fn merge<Fget, Fset>(&mut self, xs: [i32; 4], ys: [i32; 4],
                         mut fget: Fget, mut fset: Fset) -> bool
        where Fget: FnMut(&Game<R, W>, i32, i32) -> u32, Fset: FnMut(&mut Game<R, W>, i32, i32, u32) {
        let mut moved = false;  // Fuck: https://github.com/rust-lang/rust/issues/28570

        for x in xs.iter() {
            let mut prev_non_zero_y = ys[0];
            let first_y = ys[0];
            for y in (&ys[1..4]).iter() {
                let mut val = fget(self, *x, *y);
                if val != 0u32 {
                    if prev_non_zero_y < *y && fget(self, *x, prev_non_zero_y) == val {
                        let score = val * 2;
                        self.score += score;
                        fset(self, *x, prev_non_zero_y, score);
                        val = 0u32;
                        fset(self, *x, *y, val);
                        prev_non_zero_y = *y + 1;
                        moved = true;
                    } else {
                        prev_non_zero_y = *y;
                    }
                }

                let mut k = *y;
                while k > first_y {
                    let prev = fget(self, *x, k-1);
                    if prev != 0u32 || val == 0u32 {
                        break;
                    }
                    moved = true;
                    fset(self, *x, k-1, val);
                    fset(self, *x, k, 0u32);
                    k -= 1;
                    prev_non_zero_y = k;
                }
            }
        }

        moved
    }

    fn up(&mut self) -> bool {
        let fget = |g: &Game<R, W>, x: i32, y: i32| g.get_by_pos(y, x);
        let fset = |g: &mut Game<R, W>, x: i32, y: i32, val: u32| g.set_by_pos(y, x, val);
        let xs = [0, 1, 2, 3];
        let ys = [0, 1, 2, 3];
        return self.merge(xs, ys, fget, fset);
    }

    fn right(&mut self) -> bool {
        let fget = |g: &Game<R, W>, x: i32, y: i32| g.get_by_pos(-x, -y);
        let fset = |g: &mut Game<R, W>, x: i32, y: i32, val: u32| g.set_by_pos(-x, -y, val);
        let xs = [-3, -2, -1, 0];
        let ys = [-3, -2, -1, 0];
        return self.merge(xs, ys, fget, fset);
    }

    fn down(&mut self) -> bool {
        let fget = |g: &Game<R, W>, x: i32, y: i32| g.get_by_pos(-y, -x);
        let fset = |g: &mut Game<R, W>, x: i32, y: i32, val: u32| g.set_by_pos(-y, -x, val);
        let xs = [-3, -2, -1, 0];
        let ys = [-3, -2, -1, 0];
        return self.merge(xs, ys, fget, fset);
    }

    fn left(&mut self) -> bool {
        let fget = |g: &Game<R, W>, x: i32, y: i32| g.get_by_pos(x, y);
        let fset = |g: &mut Game<R, W>, x: i32, y: i32, val: u32| g.set_by_pos(x, y, val);
        let xs = [0, 1, 2, 3];
        let ys = [0, 1, 2, 3];
        return self.merge(xs, ys, fget, fset);
    }
}

fn main() {
    let stdout = io::stdout();
    let stdout = stdout.lock();
    let stdout = stdout.into_raw_mode().unwrap();
    let stdin = io::stdin();
    let stdin = stdin.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();

    let termsize = termion::terminal_size().ok();
    let (termwidth, termheight) = termsize.map(|(w, h)| (w-2, h-2)).unwrap();
    if termwidth < 32 || termheight < 20 {
        write!(stderr, "{}{}terminal size must satisfy with (width >= 32, height >= 20){}",
            style::Bold, color::Fg(color::Red), style::Reset).unwrap();
    } else {
        init(stdin, stdout);
    }
}
