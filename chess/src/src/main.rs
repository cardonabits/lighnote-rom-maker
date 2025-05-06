use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};

mod lib;
use lib::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Be verbose
    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    verbose: bool,

    /// Only count puzzles
    #[arg(long, action = clap::ArgAction::SetTrue)]
    dry_run: bool,

    /// Maximum moves in puzzle
    #[arg(long, default_value_t = 100)]
    max_moves: usize,

    /// Minimum moves in puzzle
    #[arg(long, default_value_t = 2)]
    min_moves: usize,

    /// Only include puzzles with this theme tag
    #[arg(long)]
    theme_tag: Option<String>,

    /// Maximum rating of the puzzle
    #[arg(long, default_value_t = 10000)]
    max_rating: u32,

    /// Minimum rating of the puzzle
    #[arg(long, default_value_t = 1)]
    min_rating: u32,

    /// Skip puzzles with these pieces
    #[arg(long)]
    exclude_pieces: Option<String>,

    /// Only include puzzles where last moved piece was in this set
    #[arg(long, default_value = "prnbkq")]
    last_move_pieces: String,
}

#[derive(Debug)]
struct Puzzle {
    id: String,
    fen: String,
    moves: Vec<String>,
    rating: u32,
    themes: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let max_num_pages = 16 * 1024 * 1024 / 96;

    if !args.dry_run {
        fs::create_dir_all("fenpuzzles")?;
    } else {
        println!("Dry run, no puzzles will be generated...");
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    // Skip header
    let _headers = lines.next();

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
            .progress_chars("#>-"),
    );

    let mut puzzle_count = 0;
    let mut page_count = 0;
    let mut _skipped_count = 0;

    for line in lines {
        let line = line?;
        pb.inc(1);

        let record: Vec<&str> = line.split(',').collect();
        if record.len() < 8 {
            continue;
        }

        let puzzle = Puzzle {
            id: record[0].to_string(),
            fen: record[1].to_string(),
            moves: record[2].split_whitespace().map(|s| s.to_string()).collect(),
            rating: record[3].parse()?,
            themes: record[7].split_whitespace().map(|s| s.to_string()).collect(),
        };

        // Filter puzzles
        if let Some(exclude) = &args.exclude_pieces {
            if puzzle.fen.to_lowercase().chars().any(|c| exclude.to_lowercase().contains(c)) {
                if args.verbose {
                    println!("Skipped {}: contains excluded pieces", puzzle.id);
                }
                _skipped_count += 1;
                continue;
            }
        }

        if puzzle.rating > args.max_rating || puzzle.rating < args.min_rating {
            if args.verbose {
                println!("Skipped {}: rating out of range", puzzle.id);
            }
            _skipped_count += 1;
            continue;
        }

        if puzzle.moves.len() > args.max_moves || puzzle.moves.len() < args.min_moves {
            if args.verbose {
                println!("Skipped {}: move count out of range", puzzle.id);
            }
            _skipped_count += 1;
            continue;
        }

        if let Some(theme_tag) = &args.theme_tag {
            if !puzzle.themes.iter().any(|t| t == theme_tag) {
                if args.verbose {
                    println!("Skipped {}: wrong theme", puzzle.id);
                }
                _skipped_count += 1;
                continue;
            }
        }

        puzzle_count += 1;
        page_count += puzzle.moves.len();

        if !args.dry_run {
            let current_fen = puzzle.fen.split_whitespace().next().unwrap().to_string();
            let mut moved_piece = ' ';

            for (i, move_str) in puzzle.moves.iter().enumerate() {
                let chess_move = parse_move(move_str)?;
                let (new_fen, piece) = apply_move(&current_fen, &chess_move)?;
                moved_piece = piece;

                let reversed = puzzle.fen.split_whitespace().nth(1).unwrap_or("w") == "b";
                let imove = move_to_index(move_str, reversed)?;

                let output_fen = if reversed {
                    reverse_fen(&new_fen)
                } else {
                    new_fen.clone()
                };

                let outfile = format!(
                    "fenpuzzles/puzzle-{}-{}-{}-{:02}.txt",
                    puzzle.id,
                    puzzle.rating,
                    args.theme_tag.as_deref().unwrap_or(""),
                    i + 1
                );

                fs::write(
                    outfile,
                    format!(
                        "{},{},{},{},{}",
                        puzzle.id,
                        expand_fen(&output_fen),
                        imove,
                        i + 1,
                        puzzle.moves.len()
                    ),
                )?;
            }

            // Check last moved piece
            if !args.last_move_pieces.to_lowercase().contains(moved_piece.to_ascii_lowercase()) {
                if args.verbose {
                    println!(
                        "Skipped {}: last move piece {} not in allowed set",
                        puzzle.id, moved_piece
                    );
                }
                // Remove generated files
                for i in 0..puzzle.moves.len() {
                    let outfile = format!(
                        "fenpuzzles/puzzle-{}-{}-{}-{:02}.txt",
                        puzzle.id,
                        puzzle.rating,
                        args.theme_tag.as_deref().unwrap_or(""),
                        i + 1
                    );
                    fs::remove_file(outfile).ok();
                }
                puzzle_count -= 1;
                page_count -= puzzle.moves.len();
            }
        }

        if page_count > max_num_pages {
            if args.verbose || args.dry_run {
                println!("Maximum pages limit ({}) reached", max_num_pages);
            }
            break;
        }
    }

    pb.finish_with_message("Done");

    let kbytes = page_count * 96 / 1024;
    println!("\nGenerated {} puzzles", puzzle_count);
    println!("and a total of {} screens/pages ({} KB)", page_count, kbytes);

    Ok(())
}
