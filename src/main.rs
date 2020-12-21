use std::{
    collections::HashMap,
    io::{self, Write},
};

use clap::{crate_authors, crate_version, Clap};

use regex::{Regex, RegexBuilder};

use reqwest::blocking::Client;

/// Download a websudoku puzzle by id
#[derive(Clap, Clone, Debug)]
#[clap(version = crate_version!(), author = crate_authors!())]
struct Opts {
    id: String,
}

impl Opts {
    fn url(&self) -> String {
        format!("https://grid.websudoku.com/?level=1&set_id={}", self.id)
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

    fn extract(&self, content: &str) -> Option<Puzzle> {
        static PUZZLE_ID: &str = "pid";
        static SOLUTION: &str = "cheat";
        static MASK: &str = "editmask";

        let map = self.build_extraction_map(content);

        Some(Puzzle {
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
    id: String,
    solution: Vec<u8>,
    mask: Vec<bool>,
}

impl Puzzle {
    fn write_masked_puzzle(&self, mut w: impl Write) -> io::Result<()> {
        let rows = self.solution.chunks(9).filter(|&x| x.len() == 9);
        let row_masks = self.mask.chunks(9).filter(|&x| x.len() == 9);

        for (row, mask) in rows.zip(row_masks) {
            let mut col_idx = 0u8;
            for (&value, &can_edit) in row.iter().zip(mask) {
                if col_idx == 8 {
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
                col_idx += 1;
            }
            w.write_all(b"\n")?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    static USER_AGENT: &str =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:83.0) Gecko/20100101 Firefox/83.0";

    let opts = Opts::parse();
    let extractor = PuzzleExtractor::new();
    let client = Client::builder().user_agent(USER_AGENT).build()?;

    let content = client.get(&opts.url()).send()?.text()?;
    let puzzle = extractor
        .extract(&content)
        .expect("Unable to extract puzzle data");

    write_csv(&puzzle)?;

    Ok(())
}

fn write_csv(puzzle: &Puzzle) -> io::Result<()> {
    use std::fs::File;
    puzzle.write_masked_puzzle(File::create(&format!("{}.csv", puzzle.id))?)
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
    #[test]
    fn input_regex_works() {
        let content = include_str!("../sample.html");
        let extractor = super::PuzzleExtractor::new();

        let actual = extractor.extract(content).unwrap();
        let expected = super::Puzzle {
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
