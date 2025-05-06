#! /bin/bash

# Generate flash file.  A puzzle is a series of
# text files, 75 chars each.  Each text file is aligned
# to 96 bytes (ROW_SIZE)

ROW_SIZE=96
MAX_MOVES_PER_PUZZLE=4
MAX_PUZZLE_SIZE=$((ROW_SIZE * MAX_MOVES_PER_PUZZLE))
FLASH_SIZE=16777216
CONFIG_SECTOR_SIZE=0x1000
CONFIG_STRUCT_SIZE=36

ROMFILE=lightnote.rom
rm -f ${ROMFILE}
row_count=0
puzzle_count=0

echo "Generating rom file..."
# This is important or we get puzzles with same name and different case get
# intermixed
LANG=C
for f in fenpuzzles/*.txt
do
    echo -n .
    row_count=$((row_count+1))
    # files ending in -01.txt are beginnings of puzzles
    [ ${f: -6:2} = '01' ] && puzzle_count=$((puzzle_count+1))
    cat ${f} >> ${ROMFILE}
    padded_size=$((ROW_SIZE*row_count))
    dd if=/dev/null of=${ROMFILE} obs=${padded_size} seek=1 status=none
    FREE_SPACE=$((FLASH_SIZE-CONFIG_SECTOR_SIZE-padded_size))
    [ ${FREE_SPACE} -lt ${MAX_PUZZLE_SIZE} ] && break
done
echo
echo "${puzzle_count} puzzles in ${padded_size} bytes..."

echo "Padding with ${FREE_SPACE} bytes to fill up to config sector..."
dd if=/dev/null of=${ROMFILE} obs=$((FLASH_SIZE-CONFIG_SECTOR_SIZE)) seek=1 status=none

echo "Writing config sector..."
python -c $"import sys
from struct import pack

# magic: u32 = 0x11131719
sys.stdout.buffer.write(pack('<L', 0x11131719))
# num_pages: u32 (a record is 1 page)
sys.stdout.buffer.write(pack('<L', ${row_count}))
# total_size: u32
sys.stdout.buffer.write(pack('<L', $((row_count * ROW_SIZE))))
# num_types: u8
sys.stdout.buffer.write(pack('<B', 0x1))
# font_size: u8
sys.stdout.buffer.write(pack('<B', 0x1))
# reserved0, reserved1
sys.stdout.buffer.write(pack('<H', 0x0))
# type0: u8
sys.stdout.buffer.write(pack('<B', 0x4))
# type1: u8
sys.stdout.buffer.write(pack('<B', 0x0))
# type2: u8
sys.stdout.buffer.write(pack('<B', 0x0))
# type3: u8
sys.stdout.buffer.write(pack('<B', 0x0))
# size0: u32
sys.stdout.buffer.write(pack('<L', ${ROW_SIZE}))
# size1: u32
sys.stdout.buffer.write(pack('<L', 0)) 
# size2: u32
sys.stdout.buffer.write(pack('<L', 0))
# size3: u32
sys.stdout.buffer.write(pack('<L', 0))

# fill the remaining config sector with zeros
sys.stdout.buffer.write(b'\0' * (${CONFIG_SECTOR_SIZE} - ${CONFIG_STRUCT_SIZE}))" >> ${ROMFILE}

echo Done

# Unused = 0,
# Text = 1,
# RawImage = 2,
# Sensors = 3,
# ChessPuzzle = 4

