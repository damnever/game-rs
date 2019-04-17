extern crate rand;
extern crate termion;

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use rand::Rng;
use termion::raw::IntoRawMode;
use termion::{async_stdin, clear, color, cursor, style};

mod object {
    use super::color;

    pub type Object = u8;
    pub const SPACE: Object = 0;
    pub const BARRIER: Object = 1;
    pub const SNAKE_HEAD: Object = 2;
    pub const SNAKE_BODY: Object = 3;
    pub const FOOD: Object = 4;

    pub const COLORS: [color::Bg<color::Rgb>; 5] = [
        color::Bg(color::Rgb(224, 224, 224)),
        color::Bg(color::Rgb(0, 0, 0)),
        color::Bg(color::Rgb(153, 0, 0)),
        color::Bg(color::Rgb(255, 0, 0)),
        color::Bg(color::Rgb(0, 153, 0)),
    ];
}

// TODO(damnever): make speed and game map configurable.
const SPACE_MARK: char = '.';
const BARRIER_MARK: char = '*';
const MAX_FOOD: usize = 10;
const SPEEDS: [Duration; 10] = [
    Duration::from_millis(600),
    Duration::from_millis(500),
    Duration::from_millis(400),
    Duration::from_millis(300),
    Duration::from_millis(200),
    Duration::from_millis(100),
    Duration::from_millis(80),
    Duration::from_millis(50),
    Duration::from_millis(30),
    Duration::from_millis(11),
];
const DEFAULT_MAP: &'static str = "**...............................................**
*.................................................*
...................................................
...................................................
...................................................
...................................................
...................................................
...................................................
*..................................................
*.................................................*
*.................................................*
*.................................................*
*.................................................*
*.................................................*
..................................................*
...................................................
...................................................
...................................................
...................................................
...................................................
...................................................
*.................................................*
**...............................................**
";

#[derive(Debug, Clone, Copy)]
enum Key {
    Up,
    Down,
    Left,
    Right,
    Restart,
    Quit,
}

impl Key {
    fn from(key_byte: u8, default: Key) -> Key {
        let key = key_byte as char;
        match key {
            'w' | 'k' => Key::Up,
            'd' | 'l' => Key::Right,
            's' | 'j' => Key::Down,
            'a' | 'h' => Key::Left,
            'r' => Key::Restart,
            'q' => Key::Quit,
            _ => default,
        }
    }

    fn rand_direction(rng: &mut rand::ThreadRng) -> Key {
        let directions = [Key::Up, Key::Down, Key::Left, Key::Right];
        directions[rng.gen_range(0, 4)]
    }
}

struct Game<R, W: Write> {
    init_pos: usize,
    rows: usize,
    cols: usize,
    score_to_speed: usize,
    score: usize,
    food: usize,
    spaces: usize,
    snake: VecDeque<usize>,
    map: Vec<object::Object>,
    stdin: R,
    stdout: W,
    rng: rand::ThreadRng,
}

impl<R: Read, W: Write> Game<R, W> {
    fn new(stdin: R, stdout: W, map: &mut Vec<u8>, cols: usize) -> Self {
        let init_pos = map.len() / 2;
        map[init_pos] = object::SNAKE_HEAD;
        let mut game = Game {
            init_pos: init_pos,
            rows: map.len() / cols,
            cols: cols,
            score_to_speed: map.len() / 3 / SPEEDS.len(),
            score: 0,
            food: 0,
            spaces: 0,
            snake: VecDeque::new(),
            map: map.to_owned(),
            stdin: stdin,
            stdout: stdout,
            rng: rand::thread_rng(),
        };
        game.reset();

        game
    }

    fn start(&mut self) {
        self.feed();
        self.draw();
        let mut speed = self.try_speed_up();
        let mut keys_buf = [0u8; 23]; // Whatever
        let mut prev_direction = Key::rand_direction(&mut self.rng);
        let mut start = Instant::now();

        loop {
            let keys_num = self.stdin.read(&mut keys_buf).unwrap();
            let mut key = prev_direction;
            if keys_num > 0 {
                key = Key::from(keys_buf[keys_num - 1], prev_direction);
            }

            let game_over = match key {
                Key::Up => self.up(),
                Key::Right => self.right(),
                Key::Down => self.down(),
                Key::Left => self.left(),
                Key::Restart => {
                    self.reset();
                    self.start();
                    return;
                }
                Key::Quit => return,
            };
            prev_direction = key;

            self.feed();
            self.draw();
            if game_over {
                self.draw_menu();
                // Continue to sleep to avoid busy loop.
            }

            let elapsed = start.elapsed();
            if speed > elapsed {
                sleep(speed - elapsed);
            }
            start = Instant::now();
            speed = self.try_speed_up();
        }
    }

    fn reset(&mut self) {
        self.snake.clear();
        self.snake.push_back(self.init_pos);
        self.score = 0;
        self.food = 0;
        self.spaces = 0;
        for i in 0..(self.rows * self.cols) {
            let obj = self.map[i];
            if obj != object::SPACE && obj != object::BARRIER {
                self.map[i] = object::SPACE;
            }
            if obj == object::SPACE && self.init_pos != i {
                self.spaces += 1;
            }
        }
        self.map[self.init_pos] = object::SNAKE_HEAD;
    }

    fn snake_head_pos(&self) -> (usize, usize) {
        let pos = self.snake[0];
        (pos / self.cols, pos % self.cols)
    }

    fn pos(&mut self, x: usize, y: usize) -> usize {
        let x = if x > self.rows {
            // overflow
            self.rows - 1
        } else if x == self.rows {
            0
        } else {
            x
        };
        let y = if y > self.cols {
            // overflow
            self.cols - 1
        } else if y == self.cols {
            0
        } else {
            y
        };

        x * self.cols + y
    }

    fn up(&mut self) -> bool {
        let (x, y) = self.snake_head_pos();
        self.move_to(x - 1, y)
    }

    fn down(&mut self) -> bool {
        let (x, y) = self.snake_head_pos();
        self.move_to(x + 1, y)
    }

    fn left(&mut self) -> bool {
        let (x, y) = self.snake_head_pos();
        self.move_to(x, y - 1)
    }

    fn right(&mut self) -> bool {
        let (x, y) = self.snake_head_pos();
        self.move_to(x, y + 1)
    }

    fn move_to(&mut self, x: usize, y: usize) -> bool {
        let pos = self.pos(x, y);
        match self.map[pos] {
            object::BARRIER | object::SNAKE_BODY => true,
            object::FOOD => {
                self.snake.push_front(pos);
                self.map[pos] = object::SNAKE_HEAD;
                self.map[self.snake[1]] = object::SNAKE_BODY;
                self.food -= 1;
                self.score += 1;
                false
            }
            object::SPACE => {
                self.snake.push_front(pos);
                self.map[pos] = object::SNAKE_HEAD;
                self.map[self.snake[1]] = object::SNAKE_BODY;
                let tail_pos = self.snake.pop_back().unwrap();
                self.map[tail_pos] = object::SPACE;
                false
            }
            _ => unreachable!(),
        }
    }

    fn feed(&mut self) {
        // TODO(damnever): maintain a space vector..
        while MAX_FOOD > self.food && self.spaces > 0 {
            let idx = self.rng.gen_range(0, self.map.len());
            if self.map[idx] == object::SPACE {
                self.map[idx] = object::FOOD;
                self.food += 1;
                self.spaces -= 1;
            }
        }
    }

    fn try_speed_up(&mut self) -> Duration {
        let mut idx = self.score / self.score_to_speed;
        if idx >= SPEEDS.len() {
            idx = SPEEDS.len() - 1;
        }

        SPEEDS[idx]
    }

    fn draw(&mut self) {
        write!(
            self.stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1)
        )
        .unwrap();

        let bg_border = color::Bg(color::Rgb(255, 255, 204));
        let width = (self.cols + 2) * 2;
        // Header
        let vir_line = format!(
            "{}{:spaces$}{}\n\r",
            bg_border,
            " ",
            style::Reset,
            spaces = width,
        );

        self.stdout.write(vir_line.as_bytes()).unwrap();
        let score = self.score.to_string();
        write!(
            self.stdout,
            "{}  SCORE: {}{:space$}{}\n\r",
            style::Bold,
            score,
            " ",
            style::Reset,
            space = width - 9 - score.len()
        )
        .unwrap();

        // Body
        let mut line: String = String::new();
        line.push_str(vir_line.as_str());
        self.stdout.write(line.as_bytes()).unwrap();
        for row in 0..self.rows {
            line.clear();
            line.push_str(format!("{}  {}", bg_border, style::Reset).as_str());
            for col in 0..self.cols {
                let bg = object::COLORS[self.map[row * self.cols + col] as usize];
                line.push_str(format!("{}{:2}{}", bg, " ", style::Reset).as_str());
            }
            line.push_str(format!("{}  {}\n\r", bg_border, style::Reset).as_str());
            self.stdout.write(line.as_bytes()).unwrap();
        }
        line.clear();
        line.push_str(vir_line.as_str());
        self.stdout.write(line.as_bytes()).unwrap();
        self.stdout.flush().unwrap();
    }

    fn draw_menu(&mut self) {
        let start = (self.cols / 3) as u16;
        let bg = color::Bg(color::Rgb(128, 128, 128));
        write!(
            self.stdout,
            "{}{}{}  GAME OVER!   {}\n\r",
            cursor::Goto(start, 4),
            bg,
            style::Bold,
            style::Reset
        )
        .unwrap();
        write!(
            self.stdout,
            "{}{}  restart: r   {}\n\r",
            cursor::Goto(start, 5),
            bg,
            style::Reset
        )
        .unwrap();
        write!(
            self.stdout,
            "{}{}  quit: q      {}\n\r",
            cursor::Goto(start, 6),
            bg,
            style::Reset
        )
        .unwrap();
        write!(
            self.stdout,
            "{}{}               {}\n\r",
            cursor::Goto(start, 7),
            bg,
            style::Reset
        )
        .unwrap();
        self.stdout.flush().unwrap();
    }
}

impl<R, W: Write> Drop for Game<R, W> {
    fn drop(&mut self) {
        write!(
            self.stdout,
            "{}{}{}",
            clear::All,
            style::Reset,
            cursor::Goto(1, 1)
        )
        .unwrap();
    }
}

fn read_map() -> (Vec<u8>, usize) {
    let mut map = Vec::new();
    let reader = BufReader::new(Cursor::new(DEFAULT_MAP));
    let mut cols = 0usize;

    for line in reader.lines() {
        let line = line.unwrap();
        if cols != 0 && line.len() != cols {
            panic!("column number msmatch with previous one");
        } else {
            cols = line.len();
        }
        for c in line.chars() {
            if c == SPACE_MARK {
                map.push(object::SPACE);
            } else if c == BARRIER_MARK {
                map.push(object::BARRIER);
            } else {
                panic!("unknown mark {}", c);
            }
        }
    }

    (map, cols)
}

pub fn main() {
    let (map, cols) = read_map();
    let rows = map.len() / cols;

    let stdout = io::stdout();
    let stdout = stdout.lock().into_raw_mode().unwrap();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();

    let termsize = termion::terminal_size().ok();
    let (termwidth, termheight) = termsize.map(|(w, h)| (w - 2, h - 2)).unwrap();
    if termheight < (rows + 4) as u16 || termwidth < (cols + 2) as u16 * 2 {
        write!(
            stderr,
            "{}{}terminal size must satisfy with (width >= {}, height >= {}){}",
            style::Bold,
            color::Fg(color::Red),
            rows + 4,
            (cols + 2) * 2,
            style::Reset
        )
        .unwrap();
    } else {
        let mut map = map;
        let mut g = Game::new(async_stdin(), stdout, &mut map, cols);
        g.start();
    }
}
