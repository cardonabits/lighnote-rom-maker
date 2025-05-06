function color_from_row_col() {
    local i=$1
    CELL_COLOR=( \
    "white" "black" "white" "black" "white" "black" "white" "black" \
    "black" "white" "black" "white" "black" "white" "black" "white" \
    "white" "black" "white" "black" "white" "black" "white" "black" \
    "black" "white" "black" "white" "black" "white" "black" "white" \
    "white" "black" "white" "black" "white" "black" "white" "black" \
    "black" "white" "black" "white" "black" "white" "black" "white" \
    "white" "black" "white" "black" "white" "black" "white" "black" \
    "black" "white" "black" "white" "black" "white" "black" "white" \
    )
    echo ${CELL_COLOR[$i]}
}

function reverse {
    val=$1

    for((i=${#val}-1;i>=0;i--)); do rev="$rev${val:$i:1}"; done

    echo $rev
}

function bmparray_from_fen() {
    local FEN_ARRAY=("$@")
    local EFEN=$(expand_fen ${FEN_ARRAY[@]})
    BMP_ARRAY=()
    for (( i=0; i<${#EFEN}; i++ ))
    do
        square_color=$( color_from_row_col $i )
        case ${EFEN:$i:1} in
            "1")
                BMP_ARRAY+=("$square_color-cell.bmp")
            ;;
            "p")
                BMP_ARRAY+=("black-pawn-on-$square_color.bmp")
            ;;
            "P")
                BMP_ARRAY+=("white-pawn-on-$square_color.bmp")
            ;;
            "r")
                BMP_ARRAY+=("black-rook-on-$square_color.bmp")
            ;;
            "R")
                BMP_ARRAY+=("white-rook-on-$square_color.bmp")
            ;;
            "b")
                BMP_ARRAY+=("black-bishop-on-$square_color.bmp")
            ;;
            "B")
                BMP_ARRAY+=("white-bishop-on-$square_color.bmp")
            ;;
            "n")
                BMP_ARRAY+=("black-knight-on-$square_color.bmp")
            ;;
            "N")
                BMP_ARRAY+=("white-knight-on-$square_color.bmp")
            ;;
            "k")
                BMP_ARRAY+=("black-king-on-$square_color.bmp")
            ;;
            "K")
                BMP_ARRAY+=("white-king-on-$square_color.bmp")
            ;;
            "q")
                BMP_ARRAY+=("black-queen-on-$square_color.bmp")
            ;;
            "Q")
                BMP_ARRAY+=("white-queen-on-$square_color.bmp")
            ;;

            *)
               echo "something else"
            ;;
        esac
    done
    echo ${BMP_ARRAY[@]}
}

function ord() {
  LC_CTYPE=C printf '%d' "'$1"
}

# flatten and convert all numbers to 1's so that fen is represented in exactly
# in a 64 chars string
function expand_fen() {
    local FEN_ARRAY=("$@")
    EFEN_ARRAY=()
    for line in ${FEN_ARRAY[*]}
    do
        col=0
        for (( i=0; i<${#line}; i++ ))
        do
            case ${line:$i:1} in
            "2" | "3" | "4" | "5" | "6" | "7" | "8")
                for (( j=0; j<${line:$i:1}; j++ ))
                do
                    EFEN_ARRAY+=1
                done
            ;;
            "/")
                # omit /'s
            ;;

            *)
               EFEN_ARRAY+=${line:$i:1}
            ;;
            esac

            col=$((col + 1))
        done
        row=$((row + 1))
    done
    echo ${EFEN_ARRAY[@]}
}

# compress an extended fen string into a regular fen
function compress_efen() {
    local EFEN=$1
    # add slashes
    local EFEN="${EFEN:0:8}/${EFEN:8:8}/${EFEN:16:8}/${EFEN:24:8}/${EFEN:32:8}/${EFEN:40:8}/${EFEN:48:8}/${EFEN:56:8}"

    local j
    FEN=()
    for (( i=0; i<${#EFEN}; i++ ))
    do
        j=$i
        while [[ ${EFEN:$j:1} = "1" && $j -lt ${#EFEN} ]]
        do
            j=$(($j + 1))
        done
        [ $i -eq $j ] && FEN+=${EFEN:$i:1}
        [ $i -lt $j ] && { FEN+=$((j-i)); FEN+=${EFEN:$j:1}; }
        i=$j
    done
    echo ${FEN}
}

function move_to_i() {
    local move="$1"
    local from=0
    local to=0
    local reversed="$2"

    [[ ! -z ${reversed} && ${reversed} != "reverse" ]] && { echo "move_to_i d2d4 [reverse]"; exit 1; }

    from=$(ord ${move:0:1})
    from=$(( from - 97 ))
    from=$(( from + (8 - ${move:1:1}) * 8 ))

    to=$(ord ${move:2:1})
    to=$(( to - 97 ))
    to=$(( to + (8 - ${move:3:1}) * 8 ))

    if [ ! -z ${reversed} ]
    then
        from=$((from - 63))
        [ $from -lt 0 ] && from=$((-from))
        to=$((to - 63))
        [ $to -lt 0 ] && to=$((-to))
    fi

    [ $to -le 63 ] || { echo "Invalid move"; exit 1; }
    [ $from -le 63 ] || { echo "Invalid move"; exit 1; }

    printf %02d,%02d $from $to
}

# input is a move in the form "d2d4" followed by the fen array
# new fen and moved_piece are returned on stdout
function move_fen() {
    local move="$1"
    shift
    local FEN=("$@")
    local from=0
    local to=0

    EFEN=$(expand_fen $FEN)

    from=$(ord ${move:0:1})
    from=$(( from - 97 ))
    from=$(( from + (8 - ${move:1:1}) * 8 ))

    to=$(ord ${move:2:1})
    to=$(( to - 97 ))
    to=$(( to + (8 - ${move:3:1}) * 8 ))

    [ $to -le 63 ] || { echo "Invalid move"; exit 1; }
    [ $from -le 63 ] || { echo "Invalid move"; exit 1; }


    moved_piece=${EFEN:$from:1}

    #special case: promotions are indicated by 5 letter moves, where the last
    #letter is the piece the pawn was promoted to.  We need to infer color
    #from moved_piece
    promoted_piece=$moved_piece
    if [ ! -z "${move:4:1}" ]
    then
        promoted_piece=${move:4:1}
        [ "${moved_piece}" = 'P' ] && promoted_piece=${promoted_piece^}
    fi

    EFEN="${EFEN:0:from}1${EFEN:$((from+1))}"
    EFEN="${EFEN:0:to}${promoted_piece}${EFEN:$((to+1))}"
    compress_efen ${EFEN}
    echo -n $moved_piece
}

function reverse_fen() {
    local FEN=$1
    IFS=' '
    local FEN_ARRAY=(${FEN//\// })
    REVERSED_ARRAY=()
    for line in ${FEN_ARRAY[*]}
    do
        local revline=$(reverse "$line")
        if [ ${#REVERSED_ARRAY[@]} -eq 0 ]; then
            REVERSED_ARRAY=($revline)
        else
            REVERSED_ARRAY=($revline/${REVERSED_ARRAY[@]})
        fi
    done
    echo ${REVERSED_ARRAY[@]}
}

# Function to display progress bar
progress_bar() {
    local current=$1
    local total=$2
    local width=50
    
    # Calculate percentage
    local percent=$((current * 100 / total))
    # Calculate number of blocks to display
    local progress=$((current * width / total))
    
    # Build the bar
    local bar="["
    for ((i=0; i<width; i++)); do
        if [ $i -lt $progress ]; then
            bar+="="
        else
            bar+=" "
        fi
    done
    bar+="]"
    
    # Print the progress bar
    printf "\r%s %d%%" "$bar" "$percent"
}
