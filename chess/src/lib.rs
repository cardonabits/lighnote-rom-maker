use std::fmt;

#[derive(Debug)]
pub struct Puzzle {
    pub id: String,
    pub fen: String,
    pub moves: Vec<String>,
    pub rating: u32,
    pub themes: Vec<String>,
    pub first_move: char, // 'w' or 'b'
}

impl Puzzle {
    pub fn from_csv_record(record: &csv::StringRecord) -> Result<Self, ChessError> {
        if record.len() < 8 {
            return Err(ChessError::InvalidInput);
        }

        let first_move = record[1].split_whitespace()
            .nth(1)
            .unwrap_or("w")
            .chars()
            .next()
            .unwrap();

        Ok(Puzzle {
            id: record[0].to_string(),
            fen: record[1].to_string(),
            moves: record[2].split_whitespace().map(|s| s.to_string()).collect(),
            rating: record[3].parse().map_err(|_| ChessError::InvalidInput)?,
            themes: record[7].split(',').map(|s| s.trim().to_lowercase()).collect(),
            first_move,
        })
    }
}

#[derive(Debug)]
pub struct PuzzleConfig {
    pub verbose: bool,
    pub dry_run: bool,
    pub max_moves: usize,
    pub min_moves: usize,
    pub theme_tag: Option<String>,
    pub max_rating: u32,
    pub min_rating: u32,
    pub exclude_pieces: Vec<char>,
    pub last_move_pieces: Vec<char>,
}

impl PuzzleConfig {
    pub fn should_skip_puzzle(&self, puzzle: &Puzzle) -> bool {
        // Check rating bounds
        if puzzle.rating > self.max_rating || puzzle.rating < self.min_rating {
            return true;
        }

        // Check move count - be more lenient
        if puzzle.moves.len() > self.max_moves {
            return true;
        }
        if puzzle.moves.len() < self.min_moves && puzzle.moves.len() > 0 {
            return true;
        }

        // Check excluded pieces - only look at piece characters
        let piece_chars: Vec<char> = puzzle.fen.chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect();
        if self.exclude_pieces.iter().any(|p| piece_chars.contains(p)) {
            return true;
        }

        // Check theme tag if specified
        if let Some(theme) = &self.theme_tag {
            if !puzzle.themes.iter().any(|t| t == theme) {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct ChessMove {
    pub from: u8,
    pub to: u8,
    pub promotion: Option<char>,
}

#[derive(Debug)]
pub enum ChessError {
    InvalidMove,
    InvalidFen,
    InvalidInput,
    IoError(std::io::Error),
    CsvError(csv::Error),
}

impl From<std::io::Error> for ChessError {
    fn from(err: std::io::Error) -> Self {
        ChessError::IoError(err)
    }
}

impl From<csv::Error> for ChessError {
    fn from(err: csv::Error) -> Self {
        ChessError::CsvError(err)
    }
}

impl fmt::Display for ChessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChessError::InvalidMove => write!(f, "Invalid chess move"),
            ChessError::InvalidFen => write!(f, "Invalid FEN string"),
            ChessError::InvalidInput => write!(f, "Invalid input"),
            ChessError::IoError(e) => write!(f, "IO error: {}", e),
            ChessError::CsvError(e) => write!(f, "CSV error: {}", e),
        }
    }
}

impl std::error::Error for ChessError {}

pub fn expand_fen(fen: &str) -> String {
    let mut expanded = String::with_capacity(64);
    for c in fen.chars() {
        match c {
            '1'..='8' => expanded.extend(std::iter::repeat('1').take(c.to_digit(10).unwrap() as usize)),
            '/' => continue,
            _ => expanded.push(c),
        }
    }
    // Ensure exactly 64 characters
    expanded.truncate(64);
    expanded
}

pub fn compress_fen(expanded: &str) -> String {
    // First add slashes every 8 characters
    let with_slashes = expanded.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i % 8 == 0 && i != 0 {
                Some('/')
            } else {
                None
            }.into_iter().chain(std::iter::once(c))
        })
        .collect::<String>();
    
    // Then compress consecutive '1's
    let mut compressed = String::new();
    let mut count = 0;
    
    for c in with_slashes.chars() {
        if c == '1' {
            count += 1;
        } else {
            if count > 0 {
                compressed.push_str(&count.to_string());
                count = 0;
            }
            compressed.push(c);
        }
    }
    
    // Handle trailing '1's
    if count > 0 {
        compressed.push_str(&count.to_string());
    }
    
    compressed
}

pub fn reverse_fen(fen: &str) -> String {
    // Split into ranks and reverse their order
    let mut ranks: Vec<&str> = fen.split('/').collect();
    ranks.reverse();

    // Reverse each rank's string (columns)
    ranks.iter()
        .map(|rank| rank.chars().rev().collect::<String>())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn parse_move(move_str: &str) -> Result<ChessMove, ChessError> {
    if move_str.len() < 4 {
        return Err(ChessError::InvalidMove);
    }
    
    let from_file = move_str.chars().nth(0).unwrap() as u8 - b'a';
    let from_rank = 8 - move_str.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;
    let to_file = move_str.chars().nth(2).unwrap() as u8 - b'a';
    let to_rank = 8 - move_str.chars().nth(3).unwrap().to_digit(10).unwrap() as u8;
    
    let from = (from_rank * 8 + from_file) as u8;
    let to = (to_rank * 8 + to_file) as u8;
    
    let promotion = if move_str.len() > 4 {
        Some(move_str.chars().nth(4).unwrap())
    } else {
        None
    };
    
    Ok(ChessMove { from, to, promotion })
}

pub fn apply_move(fen: &str, chess_move: &ChessMove) -> Result<(String, char), ChessError> {
    let mut expanded = expand_fen(fen);
    let from_char = expanded.chars().nth(chess_move.from as usize).ok_or(ChessError::InvalidMove)?;
    
    // Handle promotion
    let to_piece = match chess_move.promotion {
        Some(p) => {
            if from_char.is_uppercase() {
                p.to_ascii_uppercase()
            } else {
                p.to_ascii_lowercase()
            }
        }
        None => from_char,
    };
    
    // Apply the move
    expanded.replace_range(chess_move.from as usize..=chess_move.from as usize, "1");
    expanded.replace_range(chess_move.to as usize..=chess_move.to as usize, &to_piece.to_string());
    
    Ok((compress_fen(&expanded), from_char))
}

pub fn move_to_index(move_str: &str, reversed: bool) -> Result<String, ChessError> {
    if move_str.len() < 4 {
        return Err(ChessError::InvalidMove);
    }

    // Calculate from index
    let from_file = (move_str.chars().nth(0).unwrap() as u8) - b'a';
    let from_rank = move_str.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;
    let mut from = from_file + (8 - from_rank) * 8;

    // Calculate to index
    let to_file = (move_str.chars().nth(2).unwrap() as u8) - b'a';
    let to_rank = move_str.chars().nth(3).unwrap().to_digit(10).unwrap() as u8;
    let mut to = to_file + (8 - to_rank) * 8;

    if reversed {
        from = (from as i32 - 63).abs() as u8;
        to = (to as i32 - 63).abs() as u8;
    }

    Ok(format!("{:02},{:02}", from, to))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_fen() {
        let fen = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR";
        let reversed = reverse_fen(fen);
        assert_eq!(reversed, "RNBKQBNR/PPPP1PPP/8/4P3/8/8/pppppppp/rnbkqbnr");
    }

    #[test]
    fn test_move_to_index() {
        // Test normal board moves
        assert_eq!(move_to_index("a1a1", false).unwrap(), "56,56");
        assert_eq!(move_to_index("h8h8", false).unwrap(), "07,07");
        assert_eq!(move_to_index("e2e4", false).unwrap(), "52,36");
        assert_eq!(move_to_index("g1f3", false).unwrap(), "62,45");

        // Test reversed board moves
        assert_eq!(move_to_index("a1a1", true).unwrap(), "07,07");
        assert_eq!(move_to_index("h8h8", true).unwrap(), "56,56");
        assert_eq!(move_to_index("e2e4", true).unwrap(), "11,27");
        assert_eq!(move_to_index("g1f3", true).unwrap(), "01,18");

        // Test promotion moves
        assert_eq!(move_to_index("a7a8q", false).unwrap(), "08,00");
        assert_eq!(move_to_index("h2h1r", true).unwrap(), "08,00");
    }

    #[test]
    fn test_apply_move() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR";
        let chess_move = parse_move("d2d4").unwrap();
        let (new_fen, piece) = apply_move(fen, &chess_move).unwrap();
        assert_eq!(new_fen, "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR");
        assert_eq!(piece, 'P');
    }

    #[test]
    fn test_apply_move_with_promotion() {
        // White pawn on a7 ready to promote
        let fen = "8/P7/8/8/8/8/8/8";
        let chess_move = parse_move("a7a8q").unwrap();
        let (new_fen, piece) = apply_move(fen, &chess_move).unwrap();
        assert_eq!(new_fen, "Q7/8/8/8/8/8/8/8"); // Should be queen on a8
        assert_eq!(piece, 'P'); // Original piece was white pawn

        // Black pawn on h2 ready to promote  
        let fen = "8/8/8/8/8/8/7p/8";
        let chess_move = parse_move("h2h1r").unwrap();
        let (new_fen, piece) = apply_move(fen, &chess_move).unwrap();
        assert_eq!(new_fen, "8/8/8/8/8/8/8/7r"); // Should be black rook on h1
        assert_eq!(piece, 'p'); // Original piece was black pawn
    }

    #[test]
    fn test_empty_board() {
        let fen = "8/8/8/8/8/8/8/8";
        let expanded = expand_fen(fen);
        assert_eq!(expanded.len(), 64);
        assert_eq!(expanded, "1111111111111111111111111111111111111111111111111111111111111111");
        let compressed = compress_fen(&expanded);
        assert_eq!(compressed, "8/8/8/8/8/8/8/8");
    }

    #[test]
    fn test_basic_fen_roundtrip() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR";
        let expanded = expand_fen(fen);
        assert_eq!(expanded.len(), 64);
        let compressed = compress_fen(&expanded);
        assert_eq!(compressed, fen);
    }

    #[test]
    fn test_fen_moves() {
        let test_cases = vec![
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
                "d2d4",
                "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR"
            ),
            (
                "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR",
                "g8f6", 
                "rnbqkb1r/pppppppp/5n2/8/3P4/8/PPP1PPPP/RNBQKBNR"
            ),
        ];

        for (start_fen, move_str, expected_fen) in test_cases {
            let chess_move = parse_move(move_str).expect("Failed to parse move");
            let (new_fen, _) = apply_move(start_fen, &chess_move).expect("Failed to apply move");
            assert_eq!(new_fen, expected_fen, "Failed for move {} in position {}", move_str, start_fen);
        }
    }
}
