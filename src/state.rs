use color_eyre::eyre::{ensure, eyre, Context};
use color_eyre::{Report, Result};
use itertools::Itertools;
use nanorand::{Rng, WyRand};
use simple_grid::{Grid, GridIndex};
use std::fmt::Display;
use std::{cmp, iter};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Pos { x, y } = self;
        write!(f, "({x}, {y})")
    }
}

impl From<Pos> for GridIndex {
    fn from(Pos { x, y }: Pos) -> Self {
        GridIndex::new(x, y)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Selection {
    Astro,
    Robot(usize), //INVARIANT: n < num_robots. this is not checked.
}

// pub struct Grid<const ROWS: usize, const COLS: usize>(pub [[Tile; ROWS]; COLS]);

impl Pos {
    fn from_iter_pair(
        xs: impl IntoIterator<Item = usize>,
        ys: impl IntoIterator<Item = usize>,
    ) -> Box<impl Iterator<Item = Pos>> {
        iter::zip(xs, ys).map(Pos::from).into()
    }
}

impl From<(usize, usize)> for Pos {
    fn from((x, y): (usize, usize)) -> Self {
        Pos { x, y }
    }
}

impl From<[usize; 2]> for Pos {
    fn from([x, y]: [usize; 2]) -> Self {
        Pos { x, y }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Tile {
    Empty,
    Astro,
    Robot,
    Goal,
}

impl Display for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Tile::Empty => '.',
            Tile::Astro => 'A',
            Tile::Robot => 'R',
            Tile::Goal => 'X',
        };
        write!(f, "{c}")
    }
}

#[derive(Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MovementAttempt {
    Success(Pos),
    Failure,
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct State {
    pub astro: Pos,
    pub robots: Vec<Pos>,
    invariants: Invariants,
}

#[derive(Clone, Hash, Debug, Eq, PartialEq)]
pub struct Invariants {
    goal: Pos,
    rows: usize,
    cols: usize,
}

impl State {
    pub fn is_at_goal(&self) -> bool {
        self.astro == self.invariants.goal
    }

    pub fn dims(&self) -> (usize, usize) {
        (self.invariants.rows, self.invariants.cols)
    }

    pub fn tile_at(&self, pos: Pos) -> Tile {
        //note that this gives less priority to Goal,
        //which means astro and robots will draw over the goal.
        if self.astro == pos {
            Tile::Astro
        } else if self.robots.contains(&pos) {
            Tile::Robot
        } else if self.invariants.goal == pos {
            Tile::Goal
        } else {
            Tile::Empty
        }
    }

    pub fn pos_of(&self, selection: Selection) -> Pos {
        match selection {
            Selection::Astro => self.astro,
            Selection::Robot(n) => self.robots[n],
        }
    }

    pub fn pos_of_mut(&mut self, selection: Selection) -> &mut Pos {
        match selection {
            Selection::Astro => &mut self.astro,
            Selection::Robot(n) => &mut self.robots[n],
        }
    }

    pub fn num_robots(&self) -> usize {
        self.robots.len()
    }

    pub fn move_toward(&self, current_pos: Pos, direction: Direction) -> MovementAttempt {
        let mut path = self.positions_in_path(current_pos, direction).peekable();

        loop {
            //if the end of the path was reached
            let Some(pos) = path.next() else { break MovementAttempt::Failure };

            match self.tile_at(pos) {
                //if reached a tile that can't be stopped on,
                //and also couldn't stop on previous tile
                Tile::Robot | Tile::Astro => break MovementAttempt::Failure,

                //if reached a tile that can be stopped on
                Tile::Empty | Tile::Goal => {
                    let next_tile = path.peek().map(|&pos| self.tile_at(pos));

                    //...but the next tile can't be stopped on
                    if let Some(Tile::Robot | Tile::Astro) = next_tile {
                        break MovementAttempt::Success(pos);
                    }

                    //otherwise, continue checking path
                }
            }
        }
    }

    fn positions_in_path(
        &self,
        path_start: Pos,
        movement_direction: Direction,
    ) -> Box<dyn Iterator<Item = Pos>> {
        let Pos { x, y } = path_start;

        match movement_direction {
            Direction::Up => {
                let xs = iter::repeat(x);
                let ys = (0..y).rev();
                Pos::from_iter_pair(xs, ys)
            }
            Direction::Down => {
                let xs = iter::repeat(x);
                let ys = y + 1..self.invariants.rows;
                Pos::from_iter_pair(xs, ys)
            }
            Direction::Left => {
                let xs = (0..x).rev();
                let ys = iter::repeat(y);
                Pos::from_iter_pair(xs, ys)
            }
            Direction::Right => {
                let xs = x + 1..self.invariants.cols;
                let ys = iter::repeat(y);
                Pos::from_iter_pair(xs, ys)
            }
        }
    }

    pub fn from_grid(grid: &Grid<Tile>) -> Result<State> {
        let mut astro = None;
        let mut goal = None;
        let mut robots = Vec::new();

        let (cols, rows) = grid.dimensions();
        for pos in (0..cols).cartesian_product(0..rows).map(Pos::from) {
            match grid[pos] {
                Tile::Empty => (),
                Tile::Astro => {
                    ensure!(astro.is_none(), "more than one player");
                    astro = Some(pos);
                }
                Tile::Robot => robots.push(pos),
                Tile::Goal => {
                    ensure!(goal.is_none(), "more than one goal");
                    goal = Some(pos);
                }
            }
        }

        let astro = astro.ok_or_else(|| eyre!("no player"))?;
        let goal = goal.ok_or_else(|| eyre!("no goal"))?;
        let initial_state = State {
            astro,
            robots,
            invariants: Invariants { goal, rows, cols },
        };
        Ok(initial_state)
    }

    /// generates a solvable state with the specified dimensions
    pub fn new_randomized(rows: usize, cols: usize) -> Result<State> {
        let mut all_positions = (0..cols)
            .cartesian_product(0..rows)
            .map(Pos::from)
            .collect_vec();

        //we want to find the first solution that is both valid (solvable)
        //and non-trivial (not too easy).

        let mut rng = WyRand::new();
        let initial_states = iter::repeat_with(|| {
            let max_robots = cmp::max(rows, cols);
            let num_robots = rng.generate_range(0..max_robots);

            assert!(num_robots + 2 < all_positions.len());

            rng.shuffle(&mut all_positions);
            let mut shuffled = all_positions.iter().copied();

            let astro = shuffled.next().unwrap();
            let goal = shuffled.next().unwrap();
            let robots = shuffled.take(num_robots).collect();

            State {
                astro,
                robots,
                invariants: Invariants { goal, rows, cols },
            }
        });

        let candidate_validate_attempts = 5000;
        let is_non_trivial = |solution: &Vec<State>| solution.len() >= 5;

        initial_states
            .take(candidate_validate_attempts)
            .filter_map(|state| state.solve_from_here())
            .find_map(|mut solution| {
                if is_non_trivial(&solution) {
                    let initial_state_of_solution = solution.swap_remove(0);
                    Some(initial_state_of_solution)
                } else {
                    None
                }
            })
            .ok_or_else(|| eyre!("all generated positions failed validation"))
    }

    pub fn pos_changes(states: &[State]) -> impl Iterator<Item = Result<PosChange>> + '_ {
        states
            .windows(2)
            .map(|win| (&win[0], &win[1]))
            .map(PosChange::try_from)
            .map(|res| res.wrap_err("paths have different invariants"))
    }
}

#[derive(Debug, Clone)]
pub struct PosChange(pub Pos, pub Pos);

impl Display for PosChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let PosChange(s, t) = self;
        write!(f, "{s} => {t}")
    }
}

impl TryFrom<(&State, &State)> for PosChange {
    type Error = Report;

    fn try_from((s, t): (&State, &State)) -> Result<PosChange> {
        ensure!(s.invariants == t.invariants, "state invariants differ");
        ensure!(s.num_robots() == t.num_robots(), "number of robots differs");

        //returns the first difference instead of validating all positions
        if s.astro != t.astro {
            return Ok(PosChange(s.astro, t.astro));
        }

        iter::zip(&s.robots, &t.robots)
            .find_map(|(&s_pos, &t_pos)| {
                if s_pos != t_pos {
                    Some(PosChange(s_pos, t_pos))
                } else {
                    None
                }
            })
            .ok_or_else(|| eyre!("start and end states are equal"))
    }
}
