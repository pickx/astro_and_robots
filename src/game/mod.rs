pub mod solver;

use crate::state::{Direction, MovementAttempt, Pos, Selection, State, Tile};
use color_eyre::eyre::eyre;
use color_eyre::{Report, Result};
use itertools::Itertools;
use std::fmt::Display;
use std::io::Write;
use std::iter;
use termion::{clear, color, cursor, terminal_size};

#[derive(Debug)]
pub struct Game {
    moves: Vec<State>,
    selected: Selection,
    mode: Mode,
    walkthrough: SolutionWalkthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Playable,
    Walkthrough,
    GameOver,
}

impl Game {
    pub fn new(initial_state: State) -> Result<Self> {
        let solution = initial_state
            .solve_from_here()
            .ok_or_else(|| eyre!("game cannot be solved from this state"))?;

        let game = Game {
            moves: vec![initial_state],
            selected: Selection::Astro,
            mode: Mode::Playable,
            walkthrough: SolutionWalkthrough::new(solution),
        };

        Ok(game)
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn state(&self) -> &State {
        self.moves.last().expect("moves are never empty")
    }

    pub fn select_next_character(&mut self) {
        let num_robots = self.state().num_robots();
        let next = match self.selected {
            Selection::Astro if num_robots == 0 => Selection::Astro,
            Selection::Astro => Selection::Robot(0),
            Selection::Robot(n) if n + 1 == num_robots => Selection::Astro,
            Selection::Robot(n) => Selection::Robot(n + 1),
        };

        self.selected = next;
    }

    pub fn select_prev_character(&mut self) {
        let num_robots = self.state().num_robots();
        let prev = match self.selected {
            Selection::Astro if num_robots == 0 => Selection::Astro,
            Selection::Astro => Selection::Robot(num_robots - 1),
            Selection::Robot(n) if n == 0 => Selection::Astro,
            Selection::Robot(n) => Selection::Robot(n - 1),
        };

        self.selected = prev;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::Playable => Mode::Walkthrough,
            Mode::Walkthrough => Mode::Playable,
            Mode::GameOver => Mode::GameOver,
        }
    }

    fn selected_pos(&self) -> Pos {
        self.state().pos_of(self.selected)
    }

    pub fn move_toward(&self, direction: Direction) -> Action {
        let attempt = self.state().move_toward(self.selected_pos(), direction);
        Action::Movement(attempt)
    }

    pub fn move_selection_to(&mut self, new_pos: Pos) {
        let mut new_state = self.state().clone();
        *new_state.pos_of_mut(self.selected) = new_pos;

        self.moves.push(new_state);

        if self.state().is_at_goal() {
            self.mode = Mode::GameOver;
        }
    }

    pub fn restart(&mut self) {
        self.moves.truncate(1);
        self.walkthrough.current_step = 0;
        self.mode = Mode::Playable;
    }

    pub fn draw(&self, stdout: &mut impl Write) -> Result<()> {
        write!(stdout, "{}", clear::All)?;
        let terminal_size = terminal_size()?;

        match self.mode() {
            Mode::Playable | Mode::GameOver => self.draw_game_state(stdout, terminal_size)?,
            Mode::Walkthrough => self.draw_walkthrough(stdout, terminal_size)?,
        };

        stdout.flush()?;
        Ok(())
    }

    fn draw_walkthrough(&self, stdout: &mut impl Write, terminal_size: (u16, u16)) -> Result<()> {
        let (rows, cols) = self.state().dims();
        let changes: Vec<_> = State::pos_changes(&self.walkthrough.solution).try_collect()?;

        let walkthrough_labels = {
            let change_labels = changes.iter().map(ToString::to_string);
            iter::once("STARTING POSITION".to_string()).chain(change_labels)
        };
        for (i, label) in walkthrough_labels.enumerate() {
            center_cursor(stdout, terminal_size, u16::try_from(i)?)?;

            if self.walkthrough.current_step == i {
                write_colored(stdout, label, color::Green)?;
                writeln!(stdout)?;
            } else {
                writeln!(stdout, "{label}")?;
            }
        }

        let is_end_pos_of_prev_step = |pos: Pos| {
            let step = self.walkthrough.current_step;
            match step.checked_sub(1) {
                Some(prev_step) => pos == changes[prev_step].1,
                None => false,
            }
        };

        writeln!(stdout)?;
        let offset_from_top = changes.len() + 2;

        for y in 0..rows {
            let adjusted_y = u16::try_from(y + offset_from_top)?;
            center_cursor(stdout, terminal_size, adjusted_y)?;

            for x in 0..cols {
                let pos = Pos { x, y };
                let tile = self.walkthrough.state().tile_at(pos);

                if is_end_pos_of_prev_step(pos) {
                    write_colored(stdout, tile, color::Red)?;
                } else {
                    write!(stdout, "{tile}")?;
                }
            }

            writeln!(stdout, "\r")?;
        }

        Ok(())
    }

    fn draw_game_state(&self, stdout: &mut impl Write, terminal_size: (u16, u16)) -> Result<()> {
        let (rows, cols) = self.state().dims();

        for y in 0..rows {
            center_cursor(stdout, terminal_size, u16::try_from(y)?)?;

            for x in 0..cols {
                let pos = Pos { x, y };
                let tile = self.state().tile_at(pos);

                if self.mode() == Mode::GameOver && tile == Tile::Astro {
                    write_colored(stdout, tile, color::Green)?;
                } else if pos == self.selected_pos() {
                    write_colored(stdout, tile, color::Red)?;
                } else {
                    write!(stdout, "{tile}")?;
                }
            }

            writeln!(stdout, "\r")?;
        }

        Ok(())
    }

    pub fn undo(&mut self) {
        if self.moves.len() > 1 {
            self.moves.pop();
        }
    }

    pub fn walkthrough_prev(&mut self) {
        self.walkthrough.decrement();
    }

    pub fn walkthrough_next(&mut self) {
        self.walkthrough.increment();
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    Movement(MovementAttempt),

    NextCharacter,
    PrevCharacter,

    Undo,
    Restart,

    Exit,

    PrevWalkthroughStep,
    NextWalkthroughStep,
    ToggleMode,
}

#[derive(Debug, Clone)]
struct SolutionWalkthrough {
    solution: Vec<State>,
    current_step: usize,
}

impl SolutionWalkthrough {
    pub fn new(solution: Vec<State>) -> Self {
        Self {
            solution,
            current_step: 0,
        }
    }

    fn state(&self) -> &State {
        self.solution
            .get(self.current_step)
            .expect("indexing field is private")
    }

    pub fn len(&self) -> usize {
        self.solution.len()
    }

    pub fn decrement(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
        }
    }

    pub fn increment(&mut self) {
        if self.current_step < self.len() - 1 {
            self.current_step += 1;
        }
    }
}

fn center_cursor(stdout: &mut impl Write, term_dims: (u16, u16), row_offset: u16) -> Result<()> {
    let (term_cols, term_rows) = term_dims;
    let (mid_cols, mid_rows) = (term_cols / 2, term_rows / 2);

    let goto_middle = cursor::Goto(mid_cols, mid_rows + row_offset);
    write!(stdout, "{goto_middle}").map_err(Report::from)
}

fn write_colored(stdout: &mut impl Write, d: impl Display, color: impl color::Color) -> Result<()> {
    let fg = color::Fg(color);
    let color_reset = color::Fg(color::Reset);
    write!(stdout, "{fg}{d}{color_reset}").map_err(Report::from)
}
