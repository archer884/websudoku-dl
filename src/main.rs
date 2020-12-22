use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Display},
    io::{self, Write},
    str::FromStr,
};

use clap::{crate_authors, crate_version, Clap};

use regex::{Regex, RegexBuilder};

use reqwest::blocking::Client;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Difficulty {
    Easy,
    Medium,
    Hard,
    Evil,
}

impl Difficulty {
    fn level(self) -> u8 {
        match self {
            Difficulty::Easy => 1,
            Difficulty::Medium => 2,
            Difficulty::Hard => 3,
            Difficulty::Evil => 4,
        }
    }
}

impl Default for Difficulty {
    fn default() -> Self {
        Difficulty::Evil
    }
}

impl Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Difficulty::Easy => f.write_str("Easy"),
            Difficulty::Medium => f.write_str("Medium"),
            Difficulty::Hard => f.write_str("Hard"),
            Difficulty::Evil => f.write_str("Evil"),
        }
    }
}

impl FromStr for Difficulty {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "easy" => Ok(Difficulty::Easy),
            "medium" => Ok(Difficulty::Medium),
            "hard" => Ok(Difficulty::Hard),
            "evil" => Ok(Difficulty::Evil),

            _ => Err(format!("Unrecognized difficulty setting: {}", s)),
        }
    }
}

/// Download a websudoku puzzle by id
#[derive(Clap, Clone, Debug)]
#[clap(version = crate_version!(), author = crate_authors!())]
struct Opts {
    /// The difficulty level of the puzzle (default: EVIL)
    #[clap(short, long)]
    difficulty: Option<Difficulty>,

    /// A puzzle url or identifier
    puzzle: String,

    /// The path of the output file. By default, this path is <puzzle>.csv, where
    /// puzzle is the puzzle's identifier.
    path: Option<String>,
}

impl Opts {
    fn params(&mut self) -> (Difficulty, String) {
        let id_pattern = Regex::new(r#"set_id=(\d+)"#).unwrap();
        let id = match id_pattern.captures(&self.puzzle) {
            Some(captures) => Cow::from(
                captures
                    .get(1)
                    .expect("Non-optional capture group should not fail")
                    .as_str(),
            ),
            None => Cow::from(self.puzzle.replace(',', "")),
        };

        let difficulty_pattern = Regex::new(r#"level=(\d)"#).unwrap();
        let difficulty = match difficulty_pattern.captures(&self.puzzle) {
            None => self.difficulty.unwrap_or_default(),
            Some(captures) => match captures
                .get(1)
                .expect("Non-optional capture group should not fail")
                .as_str()
            {
                "1" => Difficulty::Easy,
                "2" => Difficulty::Medium,
                "3" => Difficulty::Hard,
                _ => Difficulty::Evil,
            },
        };

        (
            difficulty,
            format!(
                "https://grid.websudoku.com/?level={}&set_id={}",
                difficulty.level(),
                id
            ),
        )
    }
}

struct PuzzleExtractor {
    pattern: Regex,
}

impl PuzzleExtractor {
    fn new() -> Self {
        Self {
            pattern: input_regex(),
        }
    }

    fn extract(&self, difficulty: Difficulty, content: &str) -> Option<Puzzle> {
        static PUZZLE_ID: &str = "pid";
        static SOLUTION: &str = "cheat";
        static MASK: &str = "editmask";

        let map = self.build_extraction_map(content);

        Some(Puzzle {
            difficulty,
            id: map.get(PUZZLE_ID)?.to_string(),
            solution: map.get(SOLUTION)?.bytes().map(|u| u - b'0').collect(),
            mask: map.get(MASK)?.bytes().map(|u| u == b'1').collect(),
        })
    }

    fn build_extraction_map<'a>(&self, content: &'a str) -> HashMap<&'a str, &'a str> {
        self.pattern
            .captures_iter(content)
            .map(|x| (x.get(1).unwrap().as_str(), x.get(2).unwrap().as_str()))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Puzzle {
    difficulty: Difficulty,
    id: String,
    solution: Vec<u8>,
    mask: Vec<bool>,
}

impl Puzzle {
    fn write_masked_puzzle(&self, mut w: impl Write) -> io::Result<()> {
        struct Indexes(u8);

        impl Default for Indexes {
            fn default() -> Self {
                Indexes(1)
            }
        }

        impl Iterator for Indexes {
            type Item = u8;

            fn next(&mut self) -> Option<Self::Item> {
                match self.0 {
                    9 => {
                        self.0 = 1;
                        Some(9)
                    }

                    idx => {
                        self.0 += 1;
                        Some(idx)
                    }
                }
            }
        }

        let rows = self.solution.chunks(9).filter(|&x| x.len() == 9);
        let row_masks = self.mask.chunks(9).filter(|&x| x.len() == 9);

        for (row, mask) in rows.zip(row_masks) {
            for (idx, (&value, &can_edit)) in row.iter().zip(mask).enumerate() {
                if idx == 8 {
                    if !can_edit {
                        write!(w, "{},", value)?;
                    }
                } else {
                    if can_edit {
                        w.write_all(b",")?;
                    } else {
                        write!(w, "{},", value)?;
                    }
                }
            }
            w.write_all(b"\n")?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    static USER_AGENT: &str =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:83.0) Gecko/20100101 Firefox/83.0";

    let (difficulty, url) = Opts::parse().params();
    let extractor = PuzzleExtractor::new();
    let client = Client::builder().user_agent(USER_AGENT).build()?;

    let content = client.get(&url).send()?.text()?;
    let puzzle = extractor
        .extract(difficulty, &content)
        .expect("Unable to extract puzzle data");

    write_csv(&puzzle)?;

    Ok(())
}

fn write_csv(puzzle: &Puzzle) -> io::Result<()> {
    use std::fs::File;
    puzzle.write_masked_puzzle(File::create(&format!(
        "{} {}.csv",
        puzzle.difficulty, puzzle.id
    ))?)
}

fn input_regex() -> Regex {
    RegexBuilder::new(r#"<input.+?id="(\S+)".+?value="(\d+)""#)
        .case_insensitive(true)
        .dot_matches_new_line(true)
        .build()
        .unwrap()
}

#[cfg(test)]
mod test {
    use super::{Difficulty, Puzzle, PuzzleExtractor};

    #[test]
    fn input_regex_works() {
        let content = include_str!("../resource/sample.html");
        let extractor = PuzzleExtractor::new();

        let actual = extractor.extract(Difficulty::Evil, content).unwrap();
        let expected = Puzzle {
            difficulty: Difficulty::Evil,
            id: String::from("7042100266"),
            solution: vec![
                9, 8, 4, 2, 7, 3, 6, 5, 1, 7, 1, 5, 6, 8, 4, 9, 2, 3, 3, 2, 6, 9, 5, 1, 7, 4, 8, 8,
                4, 9, 7, 3, 2, 1, 6, 5, 6, 3, 7, 8, 1, 5, 2, 9, 4, 2, 5, 1, 4, 6, 9, 3, 8, 7, 1, 9,
                3, 5, 4, 6, 8, 7, 2, 5, 7, 2, 3, 9, 8, 4, 1, 6, 4, 6, 8, 1, 2, 7, 5, 3, 9,
            ],
            mask: vec![
                true, true, true, true, false, true, false, true, true, true, false, false, false,
                true, true, false, true, true, false, true, true, false, true, true, false, true,
                true, true, false, false, true, true, false, true, false, false, false, true,
                false, false, false, false, false, true, false, false, false, true, false, true,
                true, false, false, true, true, true, false, true, true, false, true, true, false,
                true, true, false, true, true, false, false, false, true, true, true, false, true,
                false, true, true, true, true,
            ],
        };

        assert_eq!(actual, expected);
    }
}
