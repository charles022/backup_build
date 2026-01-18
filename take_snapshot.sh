#!/bin/bash
set -e

# available snapshot directories:
#   /snapshot/{home,code}

# dont move/recreate /home/user/... instead snapshot the existing /home subvolume
subvolume_list=("/home" "$HOME/code")

converter="$(command -v convert_to_subvolume.sh || true)"
if [ -z "$converter" ]; then
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [ -x "$script_dir/convert_to_subvolume.sh" ]; then
        converter="$script_dir/convert_to_subvolume.sh"
    fi
fi
if [ -z "$converter" ]; then
    echo "convert_to_subvolume.sh not found in PATH or script directory." >&2
    exit 1
fi

# require $HOME or any parent in the list to already be a subvolume
for subvolume in "${subvolume_list[@]}"; do
    case "$HOME" in
        "$subvolume"|"$subvolume"/*)
            if ! sudo btrfs subvolume show "$subvolume" &>/dev/null; then
                echo "Path must already be a btrfs subvolume: $subvolume" >&2
                exit 1
            fi
            ;;
    esac
done

# create directories
for subvolume in "${subvolume_list[@]}"; do
    base="${subvolume%/}"
    base="${base#/}"
    base="${base//\//-}"
    sudo mkdir -p "/snapshot/$base"
done

sudo chmod 700 /snapshot

# ensure subvolumes are present
for subvolume in "${subvolume_list[@]}"; do

    base="${subvolume%/}"
    base="${base#/}"
    base="${base//\//-}"
    snapshot="/snapshot/$base/$base-$(date +%Y%m%d_%H%M)"

    ###### ensure we have the subvolume ######
    # if subvolume already exists, evaluates to true
    if sudo btrfs subvolume show "$subvolume" &>/dev/null; then
        : # pass

    # if exists as a directory, convert to a subvolume (excluding $HOME and parents)
    elif [ -d "$subvolume" ]; then
        case "$HOME" in
            "$subvolume"|"$subvolume"/*)
                echo "Refusing to convert $subvolume; $HOME or a parent must already be a subvolume." >&2
                exit 1
                ;;
            *)
                "$converter" "$subvolume"
                ;;
        esac

    # if exists as something else, exit in error
    elif [ -e "$subvolume" ]; then 
      echo "Path exists but is not a directory or subvolume: $subvolume" >&2
      exit 1
    
    else # does not exist at all
      # create fresh subvolume
      sudo btrfs subvolume create "$subvolume" >/dev/null

    fi

    # create the snapshot
    sudo btrfs subvolume snapshot -r \
        "$subvolume" \
        "$snapshot"
done
