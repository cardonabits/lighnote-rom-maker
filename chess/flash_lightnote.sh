#! /bin/bash
#

set -x
show_help() {
    echo "usage: $0 [-vhec] </dev/sdX> <lightnote.rom>"
    echo "-c: only flash config sector"
    echo "-v: verify after flash"
}

OPTIND=1  # Reset in case getopts has been used previously in the shell.
VERIFY=n
SKIP=0

FLASH_SIZE=$((16 * 1024 * 1024 ))
LBA_SIZE=4096
CONFIG_SECTOR_SIZE=${LBA_SIZE}
FLASH_SIZE_LBA=$(( FLASH_SIZE / LBA_SIZE))
CONFIG_OFFSET=$((FLASH_SIZE_LBA - 1))

while getopts "h?vc" opt; do
  case "$opt" in
    h|\?)
      show_help
      exit 0
      ;;
    v)  VERIFY=y
      ;;
    c)  SKIP=${CONFIG_OFFSET}
      ;;
  esac
done

shift $((OPTIND-1))

DEVICE=$1
FILENAME=${2:-"lightnote.rom"}

[ -b "${DEVICE}" ] || { echo "${DEVICE} does not exist"; exit 1; }

# Write all 0xff to erase entire flash
if [ ${SKIP} -eq 0 ]
then
    sudo sg_write_same --10 --ff --num 0 --lba 0 --xferlen 1 ${DEVICE}
fi

sudo sg_dd blk_sgio=1 bpt=30 skip=${SKIP} seek=${SKIP} if=${FILENAME} of=${DEVICE} bs=${LBA_SIZE} count=$((FLASH_SIZE_LBA)) --progress --progress --progress

if [ "${VERIFY}" = 'y' ]
then
    # Verify
    sudo sg_dd blk_sgio=1 bpt=30 of=verify.rom if=${DEVICE} bs=${LBA_SIZE} count=$((FLASH_SIZE_LBA)) --progress --progress --progress

    diff verify.img ${FILENAME} || { echo "FAILED to verify"; exit 2; }
fi
echo DONE
