mod game;
mod state;

use clap::{Arg, ArgAction, Command};
use color_eyre::Result;
use game::{Action, Game, Mode};
use simple_grid::Grid;
use state::{Direction, MovementAttempt, State, Tile};
use std::io::{stdin, stdout};
use termion::cursor::HideCursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

fn game_loop(initial_state: State) -> Result<()> {
    let stdin = stdin();
    let mut stdout = {
        let stdout = stdout().into_alternate_screen()?.into_raw_mode()?;
        HideCursor::from(stdout)
    };

    let mut game = Game::new(initial_state)?;
    game.draw(&mut stdout)?;

    for key in stdin.keys() {
        let key = key?;
        let action = match (key, game.mode()) {
            (Key::Up, Mode::Playable) => game.move_toward(Direction::Up),
            (Key::Down, Mode::Playable) => game.move_toward(Direction::Down),
            (Key::Left, Mode::Playable) => game.move_toward(Direction::Left),
            (Key::Right, Mode::Playable) => game.move_toward(Direction::Right),

            (Key::Char('z'), Mode::Playable) => Action::PrevCharacter,
            (Key::Char('z'), Mode::Walkthrough) => Action::PrevWalkthroughStep,

            (Key::Char('x'), Mode::Playable) => Action::NextCharacter,
            (Key::Char('x'), Mode::Walkthrough) => Action::NextWalkthroughStep,

            (Key::Char('u'), Mode::Playable) => Action::Undo,
            (Key::Char('r'), _) => Action::Restart,

            (Key::Char('w'), _) => Action::ToggleMode,

            (Key::Esc | Key::Ctrl('c'), _) => Action::Exit,

            _ => continue,
        };

        match action {
            Action::Movement(MovementAttempt::Success(new_pos)) => game.move_selection_to(new_pos),
            Action::Movement(MovementAttempt::Failure) => continue,

            Action::PrevCharacter => game.select_prev_character(),
            Action::NextCharacter => game.select_next_character(),

            Action::Restart => game.restart(),
            Action::Undo => game.undo(),

            Action::Exit => break,

            Action::PrevWalkthroughStep => game.walkthrough_prev(),
            Action::NextWalkthroughStep => game.walkthrough_next(),
            Action::ToggleMode => game.toggle_mode(),
        };

        game.draw(&mut stdout)?;
    }

    Ok(())
}

fn default_grid() -> Grid<Tile> {
    use Tile::*;

    const WIDTH: usize = 5;
    const HEIGHT: usize = 5;
    static INNER: [[Tile; HEIGHT]; WIDTH] = [
        [Robot, Empty, Robot, Empty, Robot],
        [Empty, Empty, Empty, Empty, Empty],
        [Empty, Empty, Goal, Empty, Empty],
        [Empty, Empty, Empty, Empty, Robot],
        [Empty, Astro, Empty, Empty, Empty],
    ];

    let values = INNER.iter().flatten().copied();

    Grid::new(WIDTH, HEIGHT, values.collect())
}

fn dimension_in_range(dimension: &str) -> Result<usize, String> {
    let dimension = dimension
        .parse()
        .map_err(|_| format!("`{dimension}` is not a valid number"))?;

    let acceptable = 4..=10;
    acceptable
        .contains(&dimension)
        .then_some(dimension)
        .ok_or_else(|| format!("{dimension} is out of range. acceptable range: {acceptable:?}"))
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let arg_matches = Command::new("Astro and Robots")
        .arg(
            Arg::new("rows")
                .short('r')
                .long("rows")
                .help("Number of rows in grid")
                .default_value("5")
                .value_parser(dimension_in_range),
        )
        .arg(
            Arg::new("cols")
                .short('c')
                .long("cols")
                .help("Number of columns in grid")
                .default_value("5")
                .value_parser(dimension_in_range),
        )
        .arg(
            Arg::new("default")
                .long("default")
                .help("Use the predefined default instead of randomly-generating the grid")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let [rows, cols] =
        ["rows", "cols"].map(|arg| arg_matches.get_one(arg).copied().expect("default value"));

    let initial_state = if arg_matches.get_flag("default") {
        State::from_grid(&default_grid())
    } else {
        State::new_randomized(rows, cols)
    }?;

    game_loop(initial_state)?;

    Ok(())
}
