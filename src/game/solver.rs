use super::{Direction, State};
use crate::state::{MovementAttempt, Selection};
use itertools::Itertools;
use pathfinding::prelude::bfs;

impl State {
    fn successor_of(&self, selection: Selection) -> impl IntoIterator<Item = State> + '_ {
        let current_pos = self.pos_of(selection);
        let directions = [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ];

        directions.into_iter().filter_map(move |direction| {
            let attempt = self.move_toward(current_pos, direction);

            match attempt {
                MovementAttempt::Success(new_pos) => {
                    let mut new_state = self.clone();
                    *new_state.pos_of_mut(selection) = new_pos;
                    Some(new_state)
                }
                MovementAttempt::Failure => None,
            }
        })
    }

    fn all_successors(&self) -> impl IntoIterator<Item = State> {
        let mut selections = Vec::with_capacity(self.num_robots() + 1);

        selections.push(Selection::Astro);
        let robots = (0..self.num_robots()).map(Selection::Robot);
        selections.extend(robots);

        //collecting is required here, otherwise a hidden lifetime is introduced.
        selections
            .into_iter()
            .flat_map(|selection| self.successor_of(selection))
            .collect_vec()
    }

    pub fn solve_from_here(&self) -> Option<Vec<Self>> {
        bfs(self, State::all_successors, State::is_at_goal)
    }
}
