#! /bin/bash
#
# NOTE: bash version should be newer than 4.33 for ${FOO,,} to work
# NOTE: This script requires GNU getopt
# It is the default in Linux, use brew to install on MacOS
GETOPT=/usr/bin/getopt
[ -f /usr/local/opt/gnu-getopt/bin/getopt ] && GETOPT=/usr/local/opt/gnu-getopt/bin/getopt

TEMP=$(${GETOPT} -o hv --long help,verbose,max-moves:,min-moves:,theme-tag:,max-rating:,min-rating:,exclude-pieces:,last-move-pieces:,dry-run, \
              -n 'genfenpuzzles.sh' -- "$@")

if [ $? != 0 ] ; then echo "Terminating..." >&2 ; exit 1 ; fi

# Note the quotes around '$TEMP': they are essential!
eval set -- "$TEMP"

# require extended pattern matching
shopt -s extglob

function print_usage() {
    echo "$0 <options> <FILE"
    echo "--verbose be verbose"
    echo "--min-moves: minimum moves in puzzle"
    echo "--max-moves: maximum moves in puzzle"
    echo "--theme-tag: only include puzzles with this theme tag (e.g. mate)"
    echo "--min-rating: maximun rating of the puzzle"
    echo "--max-rating: minimum rating of the puzzle"
    echo "--exclude-pieces: skip puzzles with these pieces, case insensitive (e.g. QRS)"
    echo "--last-move-pieces: only include puzzles where the last moved piece was in the given set, case insensitive (e.g. pN)"
    echo "--dry-run only count the number of puzzles"
}

VERBOSE=false
DRY_RUN=false
MAX_MOVES=100
MIN_MOVES=2
MAX_RATING=10000
MIN_RATING=1
THEME_TAG=none
EXCLUDE_PIECES=""
LAST_MOVED_PIECES="prnbkq"
while true; do
  case "$1" in
    -v | --verbose ) VERBOSE=true; shift ;;
    --dry-run ) DRY_RUN=true; shift ;;
    --max-moves ) MAX_MOVES="$2"; shift 2 ;;
    --min-moves ) MIN_MOVES="$2"; shift 2 ;;
    --max-rating ) MIN_RATING="$2"; shift 2 ;;
    --min-rating ) MAX_RATING="$2"; shift 2 ;;
    --exclude-pieces ) EXCLUDE_PIECES="$2"; shift 2 ;;
    --last-move-pieces ) LAST_MOVED_PIECES="$2"; shift 2 ;;
    ---rating ) MAX_RATING="$2"; shift 2 ;;
    --theme-tag ) THEME_TAG="$2"; shift 2 ;;
    -h | --help ) print_usage; exit 0; ;;
    -- ) echo "$1"; shift; break ;;
    * ) break ;;
  esac
done

MAX_NUM_PAGES=$(( 16 * 1024 * 1024 / 96 ))

. ./functions.sh

if [ "${DRY_RUN}" = "false" ]
then
    rm -fr ./fenpuzzles
    mkdir fenpuzzles
else
    echo "Dry run, no puzzles will be generated..."
fi 

# Get total line count for progress bar
TOTAL_LINES=$(wc -l < "${1:-/dev/stdin}")
TOTAL_LINES=$(( TOTAL_LINES - 2 ))

# Skip the first line (CSV header)
read -r _  # `_` is a throwaway variable

puzzle_count=0
page_count=0
skipped_count=0

while IFS='$\n' read -r line; do
    [ "${VERBOSE}" = "false" ] && progress_bar $(( puzzle_count + skipped_count )) ${TOTAL_LINES}

    # turn into an array, splitting by commas
    IFS=","
    PUZZLE=(${line})
    FEN=${PUZZLE[1]}

    # turn into an array, splitting by whitespace
    IFS=" "
    FULL_FEN=(${FEN})
    FEN=${FULL_FEN[0]}
    FIRST_MOVE=${FULL_FEN[1]}

    RATING=(${PUZZLE[3]})

    # filter out specific pieces, ignore case (color)
    [[ ${FEN,,} == *["${EXCLUDE_PIECES,,}"]* ]] && \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: contains pieces from blacklist"
        skipped_count=$((skipped_count + 1))
        continue
    }

    # filter out easy puzzles
    [ ${RATING} -gt ${MAX_RATING} ] && \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: too easy"
        skipped_count=$((skipped_count + 1))
        continue
    }
    # filter out hard puzzles
    [ ${RATING} -lt ${MIN_RATING} ] && \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: too hard"
        skipped_count=$((skipped_count + 1))
        continue
    }
    # turn into an array, one move per entry
    IFS=" "
    MOVES=(${PUZZLE[2]})
    # this will go in the UI, so index from 1 for intuitiveness
    move_count=1

    # filter out long puzzles
    [ ${#MOVES[@]} -gt ${MAX_MOVES} ] && \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: too long"
        skipped_count=$((skipped_count + 1))
        continue
    }

    # filter out short puzzles
    [ ${#MOVES[@]} -lt ${MIN_MOVES} ] && \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: too short"
        skipped_count=$((skipped_count + 1))
        continue
    }

    # filter out by theme
    IFS=" "
    THEMES=(${PUZZLE[7]})
    if [[ ${THEME_TAG} != "none" && ! " ${THEMES[*]} " =~ " ${THEME_TAG} " ]]; then
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: wrong theme"
        skipped_count=$((skipped_count + 1))
        continue
    fi
    puzzle_count=$(($puzzle_count+1))
    page_count=$(( $page_count + ${#MOVES[@]} ))

    for move in ${MOVES[@]}
    do
        [ "${DRY_RUN}" = "true" ] && break

        # generate new fen after applying move
        IFS=$'\n'
        # output contains two return values, this is why OUT is put into array
        OUT=($(move_fen $move $FEN))
        FEN=${OUT[0]}
        MOVED_PIECE=${OUT[1]}

        THISFEN=${FEN}
        # reverse if necessary
        if [ ${FIRST_MOVE} = 'w' ]
        then
            THISFEN=$(reverse_fen $FEN)
            REVERSE="reverse"
        else
            REVERSE=""
        fi

        EFEN=$(expand_fen $THISFEN)

        # translate move to index_from, index_to
        IMOVE=$(move_to_i $move ${REVERSE})

        cd ./output
        OUTFILE_PREFIX=../fenpuzzles/puzzle-${PUZZLE[0]}-${RATING}-${THEME_TAG}
        OUTFILE=${OUTFILE_PREFIX}-$(printf '%02d' ${move_count}).txt
        echo ${PUZZLE[0]},${EFEN},${IMOVE},${move_count},${#MOVES[@]} > ${OUTFILE}
        [ ${VERBOSE} = "true" ] && echo Processed move ${move_count} of ${PUZZLE[0]} ♙ 

        move_count=$(($move_count+1))
        cd ..

    done

    # Check for last moved piece.
    # NOTE: This filter can only be applied after all moves has been processed and requires deleting previous files.
    # Probably there is a more efficient way to do this
    [[ ${MOVED_PIECE,,} == *["${LAST_MOVED_PIECES,,}"]* ]] || \
    {
        [ ${VERBOSE} = "true" ] && echo "Skipped ${PUZZLE[0]}: last move piece ${MOVED_PIECE} not in last-move-pieces set"
        # remove this puzzle and update counters
        cd ./output
        rm -fr ${OUTFILE_PREFIX}*.txt
        puzzle_count=$(($puzzle_count-1))
        page_count=$(( $page_count - ${#MOVES[@]} ))
        cd ..
    }
    [ ${page_count} -gt ${MAX_NUM_PAGES} ] && \
    {
        [ ${VERBOSE} = "true" -o ${DRY_RUN} = "true" ] && echo "Maximum pages limit (${MAX_NUM_PAGES})"
        break
    }

done

Kbytes=$(( ${page_count}*96/1024 ))
[ ${VERBOSE} = "true" -o ${DRY_RUN} = "true" ] && echo "Generated ${puzzle_count} puzzles"
[ ${VERBOSE} = "true" -o ${DRY_RUN} = "true" ] && echo "and a total of ${page_count} screens/pages ($Kbytes KB)"
echo
